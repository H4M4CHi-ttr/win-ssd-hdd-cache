# Security policy

This project is in the design and portable-host implementation phase. There is no supported production version.

Data-integrity failures, stale reads, unsafe lifetime handling, unauthorized device control, or leakage of cached contents are security-relevant. Do not put real disk contents, memory dumps, usernames, disk serials, credentials or private repository data in public issues.

The source repository is already public by its owner's decision. A verified private vulnerability-reporting channel and responsible maintainer still need to be established (GOV01/D08); source visibility does not imply a production release or support commitment. Do not assume GitHub private reporting has already been enabled. Until a verified private channel is published, open only a minimal issue asking the repository owner for a private reporting method, without exploit details or sensitive artifacts.

For development, retain a minimal synthetic reproducer and failing trace. Stop active-cache testing on valuable data. Review docs/security-and-distribution.md and docs/validation.md.

No production support or response-time SLA is promised at this stage.
