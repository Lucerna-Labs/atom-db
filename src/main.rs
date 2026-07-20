use atom_db::{AtomDb, Bond, CognitiveConfig, CognitiveEngine, Digest, RecallReport};
use std::{env, fs, io::Write, path::Path, process, str::FromStr};

fn main() {
    if let Err(error) = run() {
        eprintln!("atom-db: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [command, path] if command == "init" => {
            let db = AtomDb::open(path)?;
            println!("initialized={}", db.path().display());
        }
        [command, path, text @ ..] if command == "put" && !text.is_empty() => {
            let mut db = AtomDb::open(path)?;
            println!("{}", db.put_atom(text.join(" ").as_bytes())?);
        }
        [command, path, input] if command == "put-file" => {
            let mut db = AtomDb::open(path)?;
            println!("{}", db.put_atom(&fs::read(input)?)?);
        }
        [command, path, identity] if command == "get" => {
            let mut db = AtomDb::open(path)?;
            let identity = Digest::from_str(identity)?;
            let bytes = db.get_atom(identity)?.ok_or("atom does not exist")?;
            std::io::stdout().write_all(&bytes)?;
        }
        [command, path, source, relation, target] if command == "bond" => {
            let mut db = AtomDb::open(path)?;
            let bond = Bond {
                source: source.parse()?,
                relation: relation.parse()?,
                target: target.parse()?,
            };
            println!("{}", db.put_bond(bond)?);
        }
        [command, path, source] if command == "bonds" => {
            let db = AtomDb::open(path)?;
            for (identity, bond) in db.bonds_from(source.parse()?) {
                println!(
                    "{identity} {} {} {}",
                    bond.source, bond.relation, bond.target
                );
            }
        }
        [command, path] if command == "stats" || command == "verify" => {
            let db = AtomDb::open(path)?;
            let stats = db.stats()?;
            println!("path={}", db.path().display());
            println!("atoms={}", stats.atoms);
            println!("bonds={}", stats.bonds);
            println!("facts={}", stats.facts);
            println!("durable_bytes={}", stats.durable_bytes);
            println!("repaired_tail_bytes={}", stats.repaired_tail_bytes);
            println!("verified=true");
        }
        [command, path] if command == "demo" => demo(path)?,
        [command, path] if command == "cognitive-demo" => cognitive_demo(path)?,
        _ => print_usage(),
    }
    Ok(())
}

fn demo(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    refuse_overwrite(path)?;
    let mut db = AtomDb::open(path)?;
    let earth = db.put_atom(b"Earth")?;
    let orbits = db.put_atom(b"orbits")?;
    let sun = db.put_atom(b"Sun")?;
    let bond = db.put_bond(Bond {
        source: earth,
        relation: orbits,
        target: sun,
    })?;
    db.sync()?;
    println!("earth={earth}\norbits={orbits}\nsun={sun}\nbond={bond}");
    println!("outgoing_from_earth={}", db.bonds_from(earth).len());
    println!("verified=true");
    Ok(())
}

fn cognitive_demo(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    refuse_overwrite(path)?;
    let mut db = AtomDb::open(path)?;
    let bank = db.put_atom(b"bank")?;
    let associated = db.put_atom(b"associated-with")?;
    let money = db.put_atom(b"money")?;
    let river = db.put_atom(b"river")?;
    let finance = db.put_atom(b"finance-context")?;
    let nature = db.put_atom(b"nature-context")?;
    let bank_money = db.put_bond(Bond {
        source: bank,
        relation: associated,
        target: money,
    })?;
    let bank_river = db.put_bond(Bond {
        source: bank,
        relation: associated,
        target: river,
    })?;
    db.put_bond(Bond {
        source: finance,
        relation: associated,
        target: money,
    })?;
    db.put_bond(Bond {
        source: nature,
        relation: associated,
        target: river,
    })?;
    db.sync()?;
    let durable_facts = db.stats()?.facts;

    let mut mind = CognitiveEngine::new(CognitiveConfig::default())?;
    let baseline_finance = mind.recall(&db, bank, &[finance], None)?;
    let baseline_nature = mind.recall(&db, bank, &[nature], None)?;
    println!("durable_facts_before_cognition={durable_facts}");
    print_contrast("baseline_finance", &baseline_finance, money, river);
    print_contrast("baseline_nature", &baseline_nature, money, river);

    for _ in 0..8 {
        mind.recall(&db, bank, &[finance], Some(money))?;
    }
    let learned_finance = mind.recall(&db, bank, &[finance], None)?;
    let learned_nature = mind.recall(&db, bank, &[nature], None)?;
    print_contrast("learned_finance", &learned_finance, money, river);
    print_contrast("learned_nature", &learned_nature, money, river);
    println!(
        "bank_money_conductance={}",
        mind.bond_conductance(bank_money)
    );
    println!(
        "bank_river_conductance={}",
        mind.bond_conductance(bank_river)
    );
    println!("feedback_observations={}", mind.observations());
    println!("promoted_hops={}", learned_finance.promoted_hops);
    println!("intersections={}", learned_finance.intersections.len());
    println!("explanation={}", learned_finance.explanation);
    println!("durable_facts_after_cognition={}", db.stats()?.facts);
    println!(
        "immutable_truth_preserved={}",
        db.stats()?.facts == durable_facts
    );
    println!("verified=true");
    Ok(())
}

fn print_contrast(label: &str, report: &RecallReport, money: Digest, river: Digest) {
    println!("{label}_money_activation={}", activation(report, money));
    println!("{label}_river_activation={}", activation(report, river));
}

fn activation(report: &RecallReport, identity: Digest) -> u64 {
    report
        .ranked
        .iter()
        .find(|atom| atom.identity == identity)
        .map_or(0, |atom| atom.activation)
}

fn refuse_overwrite(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(path).exists() {
        return Err("demo path already exists; refusing to overwrite it".into());
    }
    Ok(())
}

fn print_usage() {
    eprintln!(
        "Atom DB — dependency-free immutable fact substrate\n\n\
usage:\n  atom-db init <store>\n  atom-db put <store> <text...>\n  atom-db put-file <store> <file>\n  atom-db get <store> <atom-id>\n  atom-db bond <store> <source-id> <relation-id> <target-id>\n  atom-db bonds <store> <source-id>\n  atom-db stats <store>\n  atom-db verify <store>\n  atom-db demo <new-store>\n  atom-db cognitive-demo <new-store>"
    );
}
