# Testing Concurrent Code

Prefer barriers and channels over sleeps. If a test intentionally blocks forever,
run the scenario in a child process and enforce a watchdog from the parent. Assert
the callback payload, not merely that the child stopped.

Use lock-order analysis to broaden development coverage and stress mode to vary
schedules. Normal contributors do not need to run the complete evaluation suite.
