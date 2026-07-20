use crate::{Digest, digest};
use std::{
    collections::BTreeMap,
    fmt,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

const FILE_MAGIC: &[u8; 8] = b"ATOMDB\x01\n";
const FRAME_MAGIC: &[u8; 4] = b"ATOM";
const HEADER_LEN: usize = 88;
const MAX_PAYLOAD: u64 = 64 * 1024 * 1024;
const ATOM_KIND: u8 = 1;
const BOND_KIND: u8 = 2;
const ATOM_DOMAIN: &[u8] = b"atom-db/atom/v1\0";
const BOND_DOMAIN: &[u8] = b"atom-db/bond/v1\0";
const FRAME_DOMAIN: &[u8] = b"atom-db/frame/v1\0";

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(String),
    Corrupt { offset: u64, reason: String },
    Missing(Digest),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Invalid(reason) => write!(f, "invalid operation: {reason}"),
            Self::Corrupt { offset, reason } => {
                write!(f, "corrupt store at byte {offset}: {reason}")
            }
            Self::Missing(identity) => write!(f, "unknown atom identity {identity}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bond {
    pub source: Digest,
    pub relation: Digest,
    pub target: Digest,
}

impl Bond {
    fn encode(self) -> [u8; 96] {
        let mut out = [0; 96];
        out[..32].copy_from_slice(self.source.as_bytes());
        out[32..64].copy_from_slice(self.relation.as_bytes());
        out[64..].copy_from_slice(self.target.as_bytes());
        out
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 96 {
            return Err("a bond must contain exactly three identities".into());
        }
        let mut source = [0; 32];
        let mut relation = [0; 32];
        let mut target = [0; 32];
        source.copy_from_slice(&bytes[..32]);
        relation.copy_from_slice(&bytes[32..64]);
        target.copy_from_slice(&bytes[64..]);
        Ok(Self {
            source: Digest(source),
            relation: Digest(relation),
            target: Digest(target),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Stats {
    pub atoms: u64,
    pub bonds: u64,
    pub facts: u64,
    pub durable_bytes: u64,
    pub repaired_tail_bytes: u64,
}

#[derive(Clone, Copy, Debug)]
struct Location {
    payload_offset: u64,
    length: u64,
}

pub struct AtomDb {
    path: PathBuf,
    file: File,
    atoms: BTreeMap<Digest, Location>,
    bonds: BTreeMap<Digest, Bond>,
    outgoing: BTreeMap<Digest, Vec<Digest>>,
    next_sequence: u64,
    repaired_tail_bytes: u64,
}

impl AtomDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;
        let length = file.metadata()?.len();
        if length == 0 {
            file.write_all(FILE_MAGIC)?;
            file.sync_all()?;
        } else {
            let mut magic = [0; 8];
            if length < magic.len() as u64
                || file.read_exact(&mut magic).is_err()
                || &magic != FILE_MAGIC
            {
                return Err(Error::Corrupt {
                    offset: 0,
                    reason: "file identity or format version is invalid".into(),
                });
            }
        }
        let mut db = Self {
            path,
            file,
            atoms: BTreeMap::new(),
            bonds: BTreeMap::new(),
            outgoing: BTreeMap::new(),
            next_sequence: 0,
            repaired_tail_bytes: 0,
        };
        db.recover()?;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn put_atom(&mut self, bytes: &[u8]) -> Result<Digest, Error> {
        if bytes.len() as u64 > MAX_PAYLOAD {
            return Err(Error::Invalid(
                "atom exceeds the 64 MiB Stage 1 boundary".into(),
            ));
        }
        let identity = digest(&[ATOM_DOMAIN, bytes]);
        if self.atoms.contains_key(&identity) {
            return Ok(identity);
        }
        let location = self.append(ATOM_KIND, identity, bytes)?;
        self.atoms.insert(identity, location);
        Ok(identity)
    }

    pub fn put_bond(&mut self, bond: Bond) -> Result<Digest, Error> {
        for identity in [bond.source, bond.relation, bond.target] {
            if !self.atoms.contains_key(&identity) {
                return Err(Error::Missing(identity));
            }
        }
        let payload = bond.encode();
        let identity = digest(&[BOND_DOMAIN, &payload]);
        if self.bonds.contains_key(&identity) {
            return Ok(identity);
        }
        self.append(BOND_KIND, identity, &payload)?;
        self.bonds.insert(identity, bond);
        self.outgoing.entry(bond.source).or_default().push(identity);
        Ok(identity)
    }

    pub fn get_atom(&mut self, identity: Digest) -> Result<Option<Vec<u8>>, Error> {
        let Some(location) = self.atoms.get(&identity).copied() else {
            return Ok(None);
        };
        self.file.seek(SeekFrom::Start(location.payload_offset))?;
        let mut bytes = vec![0; location.length as usize];
        self.file.read_exact(&mut bytes)?;
        Ok(Some(bytes))
    }

    pub fn get_bond(&self, identity: Digest) -> Option<Bond> {
        self.bonds.get(&identity).copied()
    }

    pub fn bonds_from(&self, source: Digest) -> Vec<(Digest, Bond)> {
        self.outgoing
            .get(&source)
            .into_iter()
            .flatten()
            .filter_map(|identity| self.bonds.get(identity).map(|bond| (*identity, *bond)))
            .collect()
    }

    pub fn all_bonds(&self) -> Vec<(Digest, Bond)> {
        self.bonds
            .iter()
            .map(|(identity, bond)| (*identity, *bond))
            .collect()
    }

    pub fn contains_atom(&self, identity: Digest) -> bool {
        self.atoms.contains_key(&identity)
    }
    pub fn sync(&mut self) -> Result<(), Error> {
        self.file.sync_all().map_err(Error::Io)
    }

    pub fn stats(&self) -> Result<Stats, Error> {
        Ok(Stats {
            atoms: self.atoms.len() as u64,
            bonds: self.bonds.len() as u64,
            facts: self.next_sequence,
            durable_bytes: self.file.metadata()?.len(),
            repaired_tail_bytes: self.repaired_tail_bytes,
        })
    }

    fn append(&mut self, kind: u8, identity: Digest, payload: &[u8]) -> Result<Location, Error> {
        let start = self.file.seek(SeekFrom::End(0))?;
        let mut header = [0; HEADER_LEN];
        header[..4].copy_from_slice(FRAME_MAGIC);
        header[4] = kind;
        header[8..16].copy_from_slice(&self.next_sequence.to_le_bytes());
        header[16..24].copy_from_slice(&(payload.len() as u64).to_le_bytes());
        header[24..56].copy_from_slice(identity.as_bytes());
        let checksum = digest(&[FRAME_DOMAIN, &header[..56], payload]);
        header[56..88].copy_from_slice(checksum.as_bytes());
        self.file.write_all(&header)?;
        self.file.write_all(payload)?;
        self.file.sync_data()?;
        self.next_sequence += 1;
        Ok(Location {
            payload_offset: start + HEADER_LEN as u64,
            length: payload.len() as u64,
        })
    }

    fn recover(&mut self) -> Result<(), Error> {
        let file_length = self.file.metadata()?.len();
        let mut offset = FILE_MAGIC.len() as u64;
        let mut sequence = 0u64;
        while offset < file_length {
            let remaining = file_length - offset;
            if remaining < HEADER_LEN as u64 {
                return self.repair_tail(offset, file_length);
            }
            self.file.seek(SeekFrom::Start(offset))?;
            let mut header = [0; HEADER_LEN];
            self.file.read_exact(&mut header)?;
            if &header[..4] != FRAME_MAGIC {
                return Err(corrupt(offset, "frame marker is invalid"));
            }
            if header[5..8] != [0, 0, 0] {
                return Err(corrupt(offset, "reserved frame bits are nonzero"));
            }
            let kind = header[4];
            let found_sequence = u64::from_le_bytes(header[8..16].try_into().expect("fixed range"));
            let length = u64::from_le_bytes(header[16..24].try_into().expect("fixed range"));
            if found_sequence != sequence {
                return Err(corrupt(offset, "causal sequence is discontinuous"));
            }
            if length > MAX_PAYLOAD {
                return Err(corrupt(offset, "payload crosses the Stage 1 size boundary"));
            }
            let frame_length = HEADER_LEN as u64 + length;
            if remaining < frame_length {
                return self.repair_tail(offset, file_length);
            }
            let mut payload = vec![0; length as usize];
            self.file.read_exact(&mut payload)?;
            let mut identity_bytes = [0; 32];
            identity_bytes.copy_from_slice(&header[24..56]);
            let identity = Digest(identity_bytes);
            let checksum = digest(&[FRAME_DOMAIN, &header[..56], &payload]);
            if checksum.as_bytes() != &header[56..88] {
                return Err(corrupt(offset, "frame checksum does not balance"));
            }
            let expected_identity = match kind {
                ATOM_KIND => digest(&[ATOM_DOMAIN, &payload]),
                BOND_KIND => digest(&[BOND_DOMAIN, &payload]),
                _ => return Err(corrupt(offset, "fact kind is unknown")),
            };
            if identity != expected_identity {
                return Err(corrupt(
                    offset,
                    "content does not produce its recorded identity",
                ));
            }
            let location = Location {
                payload_offset: offset + HEADER_LEN as u64,
                length,
            };
            match kind {
                ATOM_KIND => {
                    if self.atoms.insert(identity, location).is_some() {
                        return Err(corrupt(offset, "duplicate atom fact"));
                    }
                }
                BOND_KIND => {
                    let bond = Bond::decode(&payload).map_err(|reason| corrupt(offset, &reason))?;
                    for member in [bond.source, bond.relation, bond.target] {
                        if !self.atoms.contains_key(&member) {
                            return Err(corrupt(offset, "bond precedes one of its atoms"));
                        }
                    }
                    if self.bonds.insert(identity, bond).is_some() {
                        return Err(corrupt(offset, "duplicate bond fact"));
                    }
                    self.outgoing.entry(bond.source).or_default().push(identity);
                }
                _ => unreachable!(),
            }
            sequence += 1;
            offset += frame_length;
        }
        self.next_sequence = sequence;
        self.file.seek(SeekFrom::End(0))?;
        Ok(())
    }

    fn repair_tail(&mut self, valid_length: u64, prior_length: u64) -> Result<(), Error> {
        self.repaired_tail_bytes = prior_length - valid_length;
        self.file.set_len(valid_length)?;
        self.file.sync_all()?;
        self.file.seek(SeekFrom::End(0))?;
        self.next_sequence = (self.atoms.len() + self.bonds.len()) as u64;
        Ok(())
    }
}

fn corrupt(offset: u64, reason: &str) -> Error {
    Error::Corrupt {
        offset,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_file(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "atom-db-{name}-{}-{nonce}.atoms",
            std::process::id()
        ))
    }

    #[test]
    fn atoms_deduplicate_and_survive_reopen() {
        let path = temp_file("reopen");
        let identity = {
            let mut db = AtomDb::open(&path).unwrap();
            let first = db.put_atom(b"the same fact").unwrap();
            assert_eq!(first, db.put_atom(b"the same fact").unwrap());
            assert_eq!(db.stats().unwrap().atoms, 1);
            first
        };
        let mut db = AtomDb::open(&path).unwrap();
        assert_eq!(db.get_atom(identity).unwrap().unwrap(), b"the same fact");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn bonds_require_and_connect_atoms() {
        let path = temp_file("bond");
        let mut db = AtomDb::open(&path).unwrap();
        let source = db.put_atom(b"earth").unwrap();
        let relation = db.put_atom(b"orbits").unwrap();
        let target = db.put_atom(b"sun").unwrap();
        let bond = Bond {
            source,
            relation,
            target,
        };
        let id = db.put_bond(bond).unwrap();
        assert_eq!(db.get_bond(id), Some(bond));
        assert_eq!(db.bonds_from(source), vec![(id, bond)]);
        drop(db);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn incomplete_tail_is_removed_but_complete_corruption_fails() {
        let path = temp_file("recovery");
        let good_len = {
            let mut db = AtomDb::open(&path).unwrap();
            db.put_atom(b"durable").unwrap();
            db.stats().unwrap().durable_bytes
        };
        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            file.write_all(b"ATOM\x01torn").unwrap();
        }
        let db = AtomDb::open(&path).unwrap();
        assert_eq!(db.stats().unwrap().repaired_tail_bytes, 9);
        assert_eq!(fs::metadata(&path).unwrap().len(), good_len);
        drop(db);
        {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            file.seek(SeekFrom::Start(FILE_MAGIC.len() as u64 + HEADER_LEN as u64))
                .unwrap();
            file.write_all(b"D").unwrap();
        }
        assert!(matches!(AtomDb::open(&path), Err(Error::Corrupt { .. })));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn missing_bond_member_fails_closed() {
        let path = temp_file("missing");
        let mut db = AtomDb::open(&path).unwrap();
        let present = db.put_atom(b"present").unwrap();
        let result = db.put_bond(Bond {
            source: present,
            relation: Digest::ZERO,
            target: present,
        });
        assert!(matches!(result, Err(Error::Missing(Digest::ZERO))));
        drop(db);
        fs::remove_file(path).unwrap();
    }
}
