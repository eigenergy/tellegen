"""Download the public Austin P3U peak case into an explicit output directory."""

import concurrent.futures
import hashlib
import json
import sys
import urllib.parse
import urllib.request
import xml.etree.ElementTree as E
from pathlib import Path

prefix = "SMART-DS/v1.0/peak/AUS/P3U/scenarios/base_peak/opendss/"
base = "https://oedi-data-lake.s3.amazonaws.com/"
root = Path(sys.argv[1]).resolve()
root.mkdir(parents=True, exist_ok=True)
ns = {"s": "http://s3.amazonaws.com/doc/2006-03-01/"}
objects = []
token = None
while True:
    query = {"list-type": "2", "prefix": prefix, "max-keys": "1000"}
    if token:
        query["continuation-token"] = token
    page = E.fromstring(
        urllib.request.urlopen(base + "?" + urllib.parse.urlencode(query)).read()
    )
    for entry in page.findall("s:Contents", ns):
        key = entry.find("s:Key", ns).text
        objects.append(
            {
                "key": key,
                "size": int(entry.find("s:Size", ns).text),
                "etag": entry.find("s:ETag", ns).text,
            }
        )
    print("listed", len(objects), sum(o["size"] for o in objects), flush=True)
    if page.find("s:IsTruncated", ns).text == "false":
        break
    token = page.find("s:NextContinuationToken", ns).text
assert sum(o["size"] for o in objects) < 2_000_000_000


def fetch(item):
    relative = item["key"][len(prefix) :]
    target = (root / relative).resolve()
    assert target.is_relative_to(root)
    if not relative or relative.endswith("/"):
        return item
    target.parent.mkdir(parents=True, exist_ok=True)
    if not target.exists() or target.stat().st_size != item["size"]:
        data = urllib.request.urlopen(
            base + urllib.parse.quote(item["key"]), timeout=90
        ).read()
        assert len(data) == item["size"]
        target.write_bytes(data)
    item["sha256"] = hashlib.sha256(target.read_bytes()).hexdigest()
    return item


with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
    result = []
    for i, item in enumerate(pool.map(fetch, objects), 1):
        result.append(item)
        if i % 100 == 0:
            print("downloaded", i, flush=True)
(root / "source-manifest.json").write_text(
    json.dumps({"prefix": prefix, "objects": result}, indent=2)
)
print("complete", root, flush=True)
