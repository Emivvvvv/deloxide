# Complexity and Memory Ordering

Cycle search is linear in the explored graph. Handoff refresh is local to one
lock's waiters. Acquire/release atomics publish metadata; the wrapped physical lock
continues to provide synchronization for protected application data.
