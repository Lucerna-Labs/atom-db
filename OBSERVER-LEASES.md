# Stage 6: observer leases

## Hypothesis

An immutable causal log still fails as a database if two authorities can append
the same next sequence. The store therefore needs one operating-system-held
writer lease while allowing any number of read-only observers to inspect
completed durable prefixes.

The lease is attached to the open file handle. Process exit or crash releases
it through the operating system, so no stale lock file can become false truth.

## Access laws

1. `open_writer` acquires the single writer lease without waiting.
2. A second writer fails with `Error::Busy` before it can write.
3. `open_read_only` never creates, truncates, repairs, or synchronizes a store.
4. Any number of observers may coexist with the writer and one another.
5. Observer mutation attempts fail before file I/O.
6. An observer ignores an incomplete frame or open cell after the last committed
   prefix and reports its byte count as provisional.
7. Only a writer may truncate an incomplete tail during recovery.
8. `refresh` replaces an observer's view with a newly verified committed
   frontier; the previous view remains stable until refresh.
9. Completed corruption still fails closed for both readers and writers.
10. Dropping or crashing the writer releases the lease through the OS.

On Windows, file share modes deny a second write handle while explicitly
allowing read handles. On Unix, Atom DB writers use a nonblocking kernel `flock`
lease; observers open without taking that exclusive writer lane. Both paths use
the standard library and direct operating-system facilities, with no package
dependency or persistent sidecar lock.

## CLI behavior

- Mutating commands and `verify` acquire a writer lease.
- `get`, `bonds`, and `stats` use read-only observer handles.
- `stats` can therefore run while an application writer remains active.
- `verify` retains authority to repair an incomplete tail and fails if another
  writer is active.

## Falsification gates

Stage 6 fails if:

1. two Atom DB writer processes coexist;
2. writer contention changes the file;
3. a reader cannot coexist with an active writer;
4. two readers cannot coexist;
5. a reader exposes a provisional cell;
6. a reader truncates or repairs any bytes;
7. an observer can call a mutation successfully;
8. refresh exposes anything other than a verified frontier;
9. killing the writer leaves a stale lease;
10. earlier Stage 1-5 stores stop reopening.

## First release experiment

The release demonstration opened a writer and observer together, rejected a
second writer, wrote another fact, proved that the existing observer remained
stable, refreshed it to the new frontier, released the writer, and acquired a
replacement writer lease.

The integration test used separate operating-system processes. While a writer
process was held open, a second writer process failed and two reader processes
succeeded. The writer was then forcibly killed; a replacement writer acquired
the lease while a reader remained alive.

## Honest boundary

Stage 6 protects cooperation among Atom DB processes. Windows sharing also
prevents unrelated processes from opening a conflicting write handle. Unix
`flock` is advisory, so a program that deliberately ignores Atom DB's protocol
can still write the file directly. Observer refresh currently reconstructs the
index from the durable prefix. Directional indexes and verified checkpoints are
the next performance boundary.
