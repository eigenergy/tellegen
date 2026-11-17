"""Compare an actual Tellegen prepared matrix at a selected generated size.

Arguments: Rust test executable, KLU library, compiled klu_metrics library,
output directory, and number of feeder sections (two unknowns per section).
"""

import json
import sys
import time
from pathlib import Path

import compare


def run():
    executable, library, _metrics_library, destination, sections = sys.argv[1:6]
    sections = int(sections)
    if sections < 1:
        raise ValueError("sections must be positive")
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    start = time.perf_counter()
    result = compare.compare(
        executable,
        library,
        str(destination),
        f"tellegen-reduced-{2 * sections}",
        sections,
        direct=True,
    )
    result["total_comparison_seconds"] = time.perf_counter() - start
    (destination / "results.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    run()
