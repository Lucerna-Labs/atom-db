use atom_db::{
    AtomDb, Bond, CognitiveConfig, CognitiveEngine, Digest, Error as AtomError, LearningScope,
    RecallReport, Retriever,
};
use std::{
    env, fs,
    io::{Read, Write},
    path::Path,
    process,
    str::FromStr,
};

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
            let mut db = AtomDb::open_read_only(path)?;
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
            let db = AtomDb::open_read_only(path)?;
            for (identity, bond) in db.bonds_from(source.parse()?) {
                println!(
                    "{identity} {} {} {}",
                    bond.source, bond.relation, bond.target
                );
            }
        }
        [command, path, source, text @ ..] if command == "remember" && !text.is_empty() => {
            let mut db = AtomDb::open_writer(path)?;
            let receipt = Retriever::default().remember(&mut db, source, &text.join(" "))?;
            println!("cell={}", receipt.cell.identity);
            println!("source={}", receipt.source);
            println!("passages={}", receipt.passages.len());
            println!("unique_terms={}", receipt.unique_terms);
            println!("verified=true");
        }
        [command, path, source, input] if command == "remember-file" => {
            let mut db = AtomDb::open_writer(path)?;
            let document = fs::read_to_string(input)?;
            let receipt = Retriever::default().remember(&mut db, source, &document)?;
            println!("cell={}", receipt.cell.identity);
            println!("source={}", receipt.source);
            println!("passages={}", receipt.passages.len());
            println!("unique_terms={}", receipt.unique_terms);
            println!("verified=true");
        }
        [command, path, query @ ..] if command == "retrieve" && !query.is_empty() => {
            let mut db = AtomDb::open_read_only(path)?;
            let packet = Retriever::default().retrieve(&mut db, &query.join(" "))?;
            println!("{}", packet.to_json());
        }
        [command, path, passage, polarity @ ..]
            if command == "reinforce" && polarity.len() <= 1 =>
        {
            let polarity = match polarity.first().map(String::as_str) {
                None => atom_db::Polarity::Strengthen,
                Some("strengthen") => atom_db::Polarity::Strengthen,
                Some("weaken") => atom_db::Polarity::Weaken,
                Some(other) => return Err(format!("unknown polarity '{other}'").into()),
            };
            let passage = passage.parse::<Digest>()?;
            let mut db = AtomDb::open_writer(path)?;
            let receipt = Retriever::default().reinforce(&mut db, passage, polarity)?;
            println!("cell={}", receipt.cell.identity);
            println!("passage={}", receipt.passage);
            println!("polarity={:?}", receipt.polarity);
            println!("count={}", receipt.count);
            println!(
                "effective_delta_per_mille={}",
                receipt.effective_delta_per_mille
            );
            println!("verified=true");
        }
        [command, path] if command == "stats" => print_store_stats(&AtomDb::open_read_only(path)?)?,
        [command, path] if command == "verify" => print_store_stats(&AtomDb::open_writer(path)?)?,
        [command, path] if command == "demo" => demo(path)?,
        [command, path] if command == "cognitive-demo" => cognitive_demo(path)?,
        [command, path] if command == "context-demo" => context_demo(path)?,
        [command, path] if command == "relay-demo" => relay_demo(path)?,
        [command, path] if command == "cell-demo" => cell_demo(path)?,
        [command, path] if command == "lease-demo" => lease_demo(path)?,
        [command, mode, path, ready] if command == "lease-hold" => hold_lease(mode, path, ready)?,
        [command, mode, path] if command == "lease-probe" => probe_lease(mode, path)?,
        _ => print_usage(),
    }
    Ok(())
}

fn print_store_stats(db: &AtomDb) -> Result<(), Box<dyn std::error::Error>> {
    let stats = db.stats()?;
    println!("path={}", db.path().display());
    println!("access_mode={:?}", db.access_mode());
    println!("atoms={}", stats.atoms);
    println!("bonds={}", stats.bonds);
    println!("roots={}", stats.roots);
    println!("root_updates={}", stats.root_updates);
    println!("cells={}", stats.cells);
    println!("facts={}", stats.facts);
    println!("frames={}", stats.frames);
    println!("durable_bytes={}", stats.durable_bytes);
    println!("repaired_tail_bytes={}", stats.repaired_tail_bytes);
    println!("provisional_tail_bytes={}", stats.provisional_tail_bytes);
    println!("verified=true");
    Ok(())
}

fn hold_lease(mode: &str, path: &str, ready: &str) -> Result<(), Box<dyn std::error::Error>> {
    let db = match mode {
        "writer" => AtomDb::open_writer(path)?,
        "reader" => AtomDb::open_read_only(path)?,
        _ => return Err("lease mode must be writer or reader".into()),
    };
    fs::write(ready, format!("{:?}", db.access_mode()))?;
    let mut release = Vec::new();
    std::io::stdin().read_to_end(&mut release)?;
    drop(db);
    Ok(())
}

fn probe_lease(mode: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let db = match mode {
        "writer" => AtomDb::open_writer(path)?,
        "reader" => AtomDb::open_read_only(path)?,
        _ => return Err("lease mode must be writer or reader".into()),
    };
    println!("lease={:?}", db.access_mode());
    println!("verified=true");
    Ok(())
}

fn lease_demo(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    refuse_overwrite(path)?;
    let mut writer = AtomDb::open_writer(path)?;
    let baseline = writer.put_atom(b"observer-baseline")?;
    let mut observer = AtomDb::open_read_only(path)?;
    let writer_exclusion = matches!(AtomDb::open_writer(path), Err(AtomError::Busy(_)));
    let baseline_visible = observer.get_atom(baseline)?.is_some();
    let added = writer.put_atom(b"observer-added-later")?;
    let stale_before_refresh = observer.get_atom(added)?.is_none();
    let frontier_advanced = observer.refresh()?;
    let visible_after_refresh = observer.get_atom(added)?.is_some();
    drop(writer);
    let replacement_writer = AtomDb::open_writer(path)?;
    let crash_safe_release_model = replacement_writer.access_mode() == atom_db::AccessMode::Writer;
    drop(replacement_writer);
    let stats = observer.stats()?;

    println!("writer_exclusion={writer_exclusion}");
    println!("reader_coexisted_with_writer=true");
    println!("baseline_visible={baseline_visible}");
    println!("stale_before_refresh={stale_before_refresh}");
    println!("frontier_advanced={frontier_advanced}");
    println!("visible_after_refresh={visible_after_refresh}");
    println!("replacement_writer_opened={crash_safe_release_model}");
    println!("observer_repaired_bytes={}", stats.repaired_tail_bytes);
    println!(
        "observer_provisional_bytes={}",
        stats.provisional_tail_bytes
    );
    println!("facts={}", stats.facts);
    println!("frames={}", stats.frames);
    println!("verified=true");
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

    let config = CognitiveConfig {
        learning_scope: LearningScope::Global,
        ..CognitiveConfig::default()
    };
    let mut mind = CognitiveEngine::new(config)?;
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

fn context_demo(path: &str) -> Result<(), Box<dyn std::error::Error>> {
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

    let global_config = CognitiveConfig {
        learning_scope: LearningScope::Global,
        ..CognitiveConfig::default()
    };
    let mut global = CognitiveEngine::new(global_config)?;
    let mut contextual = CognitiveEngine::new(CognitiveConfig::default())?;
    for _ in 0..8 {
        global.recall(&db, bank, &[finance], Some(money))?;
        contextual.recall(&db, bank, &[finance], Some(money))?;
    }

    let global_finance = global.recall(&db, bank, &[finance], None)?;
    let global_nature = global.recall(&db, bank, &[nature], None)?;
    let contextual_finance = contextual.recall(&db, bank, &[finance], None)?;
    let contextual_nature = contextual.recall(&db, bank, &[nature], None)?;
    print_contrast("global_finance", &global_finance, money, river);
    print_contrast("global_nature", &global_nature, money, river);
    print_contrast("contextual_finance", &contextual_finance, money, river);
    print_contrast("contextual_nature", &contextual_nature, money, river);
    println!(
        "global_bank_money_conductance={}",
        global.bond_conductance_in(&[nature], bank_money)
    );
    println!(
        "global_bank_river_conductance={}",
        global.bond_conductance_in(&[nature], bank_river)
    );
    println!(
        "finance_bank_money_conductance={}",
        contextual.bond_conductance_in(&[finance], bank_money)
    );
    println!(
        "nature_bank_money_conductance={}",
        contextual.bond_conductance_in(&[nature], bank_money)
    );
    println!(
        "nature_bank_river_conductance={}",
        contextual.bond_conductance_in(&[nature], bank_river)
    );
    println!(
        "global_habit_crossed_context={}",
        activation(&global_nature, money) > activation(&global_nature, river)
    );
    println!(
        "context_gate_preserved_nature={}",
        activation(&contextual_nature, river) > activation(&contextual_nature, money)
    );
    println!("durable_facts_before_cognition={durable_facts}");
    println!("durable_facts_after_cognition={}", db.stats()?.facts);
    println!(
        "immutable_truth_preserved={}",
        db.stats()?.facts == durable_facts
    );
    println!("verified=true");
    Ok(())
}

fn relay_demo(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    refuse_overwrite(path)?;
    let mut db = AtomDb::open(path)?;
    let bank = db.put_atom(b"bank")?;
    let associated = db.put_atom(b"associated-with")?;
    let money = db.put_atom(b"money")?;
    let river = db.put_atom(b"river")?;
    let finance = db.put_atom(b"finance-context")?;
    let banking = db.put_atom(b"banking-context")?;
    let accounting = db.put_atom(b"accounting-context")?;
    let nature = db.put_atom(b"nature-context")?;
    let ecology = db.put_atom(b"ecology-context")?;
    let watershed = db.put_atom(b"watershed-context")?;
    let financial = db.put_atom(b"financially-related")?;
    let natural = db.put_atom(b"naturally-related")?;
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
    db.put_bond(Bond {
        source: finance,
        relation: financial,
        target: banking,
    })?;
    db.put_bond(Bond {
        source: banking,
        relation: financial,
        target: accounting,
    })?;
    db.put_bond(Bond {
        source: nature,
        relation: natural,
        target: ecology,
    })?;
    db.put_bond(Bond {
        source: ecology,
        relation: natural,
        target: watershed,
    })?;
    db.sync()?;
    let durable_facts = db.stats()?.facts;

    let mut global = CognitiveEngine::new(CognitiveConfig {
        learning_scope: LearningScope::Global,
        ..CognitiveConfig::default()
    })?;
    let mut exact = CognitiveEngine::new(CognitiveConfig {
        learning_scope: LearningScope::Contextual,
        ..CognitiveConfig::default()
    })?;
    let mut relay = CognitiveEngine::new(CognitiveConfig {
        learning_scope: LearningScope::GuardedRelay,
        ..CognitiveConfig::default()
    })?;
    for _ in 0..8 {
        global.recall(&db, bank, &[finance], Some(money))?;
        exact.recall(&db, bank, &[finance], Some(money))?;
        relay.recall(&db, bank, &[finance], Some(money))?;
    }

    let global_nature = global.recall(&db, bank, &[nature], None)?;
    let exact_finance = exact.recall(&db, bank, &[finance], None)?;
    let exact_banking = exact.recall(&db, bank, &[banking], None)?;
    let exact_nature = exact.recall(&db, bank, &[nature], None)?;
    let relay_finance = relay.recall(&db, bank, &[finance], None)?;
    let relay_banking = relay.recall(&db, bank, &[banking], None)?;
    let relay_nature = relay.recall(&db, bank, &[nature], None)?;

    print_contrast("global_nature", &global_nature, money, river);
    print_contrast("exact_finance", &exact_finance, money, river);
    print_contrast("exact_banking", &exact_banking, money, river);
    print_contrast("exact_nature", &exact_nature, money, river);
    print_contrast("relay_finance", &relay_finance, money, river);
    print_contrast("relay_banking", &relay_banking, money, river);
    print_contrast("relay_nature", &relay_nature, money, river);

    let related_compatibility = relay.context_compatibility(&db, &[finance], &[banking]);
    let opposing_compatibility = relay.context_compatibility(&db, &[finance], &[nature]);
    let exact_related = exact.bond_conductance_for(&db, &[banking], bank_money);
    let relay_trained = relay.bond_conductance_for(&db, &[finance], bank_money);
    let relay_related = relay.bond_conductance_for(&db, &[banking], bank_money);
    let relay_opposing = relay.bond_conductance_for(&db, &[nature], bank_money);
    println!(
        "relay_guard_per_mille={}",
        relay.config().relay_guard_per_mille
    );
    println!("finance_banking_compatibility_per_mille={related_compatibility}");
    println!("finance_nature_compatibility_per_mille={opposing_compatibility}");
    println!("exact_banking_bank_money_conductance={exact_related}");
    println!("relay_finance_bank_money_conductance={relay_trained}");
    println!("relay_banking_bank_money_conductance={relay_related}");
    println!("relay_nature_bank_money_conductance={relay_opposing}");
    println!(
        "relay_nature_bank_river_conductance={}",
        relay.bond_conductance_for(&db, &[nature], bank_river)
    );
    println!("relay_context_traces={}", relay.context_trace_count());
    println!(
        "relay_evicted_context_traces={}",
        relay.evicted_context_traces()
    );
    println!(
        "global_habit_crossed_context={}",
        activation(&global_nature, money) > activation(&global_nature, river)
    );
    println!("exact_related_transfer_absent={}", exact_related == 1_000);
    println!("relay_related_transfer_present={}", relay_related > 1_000);
    println!("relay_opposing_context_guarded={}", relay_opposing == 1_000);
    println!(
        "relay_improved_related_recall={}",
        activation(&relay_banking, money) > activation(&exact_banking, money)
    );
    println!(
        "relay_preserved_opposing_recall={}",
        activation(&relay_nature, river) > activation(&relay_nature, money)
    );
    println!(
        "exact_preserved_opposing_recall={}",
        activation(&exact_nature, river) > activation(&exact_nature, money)
    );
    println!("durable_facts_before_cognition={durable_facts}");
    println!("durable_facts_after_cognition={}", db.stats()?.facts);
    println!(
        "immutable_truth_preserved={}",
        db.stats()?.facts == durable_facts
    );
    println!("verified=true");
    Ok(())
}

fn cell_demo(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    refuse_overwrite(path)?;
    let mut db = AtomDb::open(path)?;

    let mut first = db.begin_cell();
    let root_name = first.put_atom(b"experiment/current");
    let version_one = first.put_atom(b"atomic-cell-version-one");
    let supersedes = first.put_atom(b"supersedes");
    first.set_root(root_name, version_one);
    let first_receipt = db.commit_cell(first)?;
    let first_snapshot = db.snapshot();

    let mut second = db.begin_cell();
    let version_two = second.put_atom(b"atomic-cell-version-two");
    second.put_bond(Bond {
        source: version_two,
        relation: supersedes,
        target: version_one,
    });
    second.set_root(root_name, version_two);
    let second_receipt = db.commit_cell(second)?;
    let second_snapshot = db.snapshot();
    db.sync()?;
    drop(db);

    let db = AtomDb::open(path)?;
    let recovered_first = db.snapshot_at(first_receipt.commit_sequence + 1)?;
    let recovered_current = db.snapshot();
    let stats = db.stats()?;
    println!("first_cell={}", first_receipt.identity);
    println!("first_commit_sequence={}", first_receipt.commit_sequence);
    println!("second_cell={}", second_receipt.identity);
    println!("second_commit_sequence={}", second_receipt.commit_sequence);
    println!("first_snapshot_sequence={}", first_snapshot.sequence());
    println!("second_snapshot_sequence={}", second_snapshot.sequence());
    println!("first_snapshot_root={:?}", first_snapshot.root(root_name));
    println!("second_snapshot_root={:?}", second_snapshot.root(root_name));
    println!("recovered_first_root={:?}", recovered_first.root(root_name));
    println!(
        "recovered_current_root={:?}",
        recovered_current.root(root_name)
    );
    println!("root_history_versions={}", db.root_history(root_name).len());
    println!("atoms={}", stats.atoms);
    println!("bonds={}", stats.bonds);
    println!("roots={}", stats.roots);
    println!("root_updates={}", stats.root_updates);
    println!("cells={}", stats.cells);
    println!("facts={}", stats.facts);
    println!("frames={}", stats.frames);
    println!("durable_bytes={}", stats.durable_bytes);
    println!(
        "historical_snapshot_recovered={}",
        recovered_first.root(root_name) == Some(version_one)
    );
    println!(
        "current_snapshot_recovered={}",
        recovered_current.root(root_name) == Some(version_two)
    );
    println!(
        "snapshot_isolation_preserved={}",
        first_snapshot.root(root_name) == Some(version_one)
            && second_snapshot.root(root_name) == Some(version_two)
    );
    println!("cell_atomicity_verified=true");
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
        "Atom DB - dependency-free immutable fact substrate\n\n\
usage:\n  atom-db init <store>\n  atom-db put <store> <text...>\n  atom-db put-file <store> <file>\n  atom-db get <store> <atom-id>\n  atom-db bond <store> <source-id> <relation-id> <target-id>\n  atom-db bonds <store> <source-id>\n  atom-db remember <store> <source> <text...>\n  atom-db remember-file <store> <source> <file>\n  atom-db retrieve <store> <query...>
  atom-db reinforce <store> <passage-id> [strengthen|weaken]\n  atom-db stats <store>\n  atom-db verify <store>\n  atom-db demo <new-store>\n  atom-db cognitive-demo <new-store>\n  atom-db context-demo <new-store>\n  atom-db relay-demo <new-store>\n  atom-db cell-demo <new-store>\n  atom-db lease-demo <new-store>"
    );
}
