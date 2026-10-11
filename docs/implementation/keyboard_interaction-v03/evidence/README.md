# Implementation evidence

This directory initially contains only a template. Copy it to `S00.json`, then one record per executed slice. All results must be real and tied to an actual commit and environment. Store sanitised logs/raw fixture measurements at relative paths underneath `evidence/`.

Record every required TEST_MATRIX evidence level, not just the strongest-sounding label. Missing Wayland/IME/accounts is `not_run` or `unavailable`, not `passed`. Offscreen native Qt tests do not count as native-wayland. Source inspection does not count as a WebEngine fixture.

Research `no_go` needs a concrete supported-API/behaviour result, not merely absent infrastructure. A completed research report is not a shipped feature. Conditional S21 can be not_applicable only with S20 no_go evidence.

Update `progress.json` after recording the evidence. The checker validates references and completeness, not whether observations were honestly recorded or whether code is correct. Raw private page text, credentials, typed input and tokenised private URLs must not be copied into logs.
