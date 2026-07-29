# Direct Wait-For Graph

Deloxide stores direct `Thread -> Thread` edges for traversal and mode-aware
`Thread -> Lock` wait intents for repair. The reverse waiter index answers “which
threads must be refreshed when this lock changes owner?” without scanning all
threads.
