# Mutex, RwLock, and Condvar

Mutex has one incompatible owner. RwLock reads conflict only with a writer; writes
conflict with the writer and every counted reader. Condvar wait releases Mutex
ownership, records the condition wait, and restores real ownership only after the
physical mutex is reacquired.
