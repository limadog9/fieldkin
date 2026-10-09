"""Build the bundled Cupid WordNet 3.0 index (Python is build tooling only).

Source: https://raw.githubusercontent.com/nltk/nltk_data/gh-pages/packages/corpora/wordnet.zip
SHA256: cbda5ea6eef7f36a97a43d4a75f85e07fccbb4f23657d27b4ccbc93e2646ab59
Run: python scripts/build_wordnet.py [path/to/wordnet.zip]
No NLTK, NumPy, Python process, network or external corpus is used by Rust.
"""

import gzip
import hashlib
from collections import deque
from functools import lru_cache
from pathlib import Path
import struct
import sys
import urllib.request
import zipfile

URL = "https://raw.githubusercontent.com/nltk/nltk_data/gh-pages/packages/corpora/wordnet.zip"
SHA256 = "cbda5ea6eef7f36a97a43d4a75f85e07fccbb4f23657d27b4ccbc93e2646ab59"
ROOT = Path(__file__).resolve().parent.parent
POS = {"noun": "n", "verb": "v", "adj": "a", "adv": "r"}
RULES = {
    "n": [("s", ""), ("ses", "s"), ("ves", "f"), ("xes", "x"), ("zes", "z"),
          ("ches", "ch"), ("shes", "sh"), ("men", "man"), ("ies", "y")],
    "v": [("s", ""), ("ies", "y"), ("es", "e"), ("es", ""), ("ed", "e"),
          ("ed", ""), ("ing", "e"), ("ing", "")],
    "a": [("er", ""), ("est", ""), ("er", "e"), ("est", "e")], "r": [],
}


def build(archive):
    index, exceptions, records = {}, {}, {}
    with zipfile.ZipFile(archive) as z:
        for file_pos, pos in POS.items():
            for line in z.read(f"wordnet/index.{file_pos}").decode().splitlines():
                if not line or line[0].isspace():
                    continue
                parts = line.split()
                start = 6 + int(parts[3])
                index.setdefault(parts[0], {})[pos] = [int(n) for n in parts[start:]]
            exceptions[pos] = {}
            for line in z.read(f"wordnet/{file_pos}.exc").decode().splitlines():
                word, *forms = line.split()
                exceptions[pos][word] = forms
            for line in z.read(f"wordnet/data.{file_pos}").decode().splitlines():
                if not line or line[0].isspace():
                    continue
                parts = line.split("|")[0].split()
                offset, actual_pos, count = int(parts[0]), parts[2], int(parts[3], 16)
                first = parts[4].split("(")[0].lower()
                sense = index[first][pos].index(offset) + 1
                name = f"{first}.{actual_pos}.{sense:02d}"
                start = 4 + 2 * count
                parents = []
                for p in range(int(parts[start])):
                    symbol, parent_offset, parent_pos, lexical = parts[start + 1 + 4*p:start + 5 + 4*p]
                    if symbol in ("@", "@i") and lexical == "0000":
                        parents.append(("a" if parent_pos == "s" else parent_pos, int(parent_offset)))
                records[pos, offset] = (name, actual_pos, parents)

        (ROOT / "assets").mkdir(exist_ok=True)
        (ROOT / "assets/WORDNET-LICENSE").write_bytes(z.read("wordnet/LICENSE"))

    ordered = sorted(records, key=lambda k: records[k][0])
    ids = {key: i for i, key in enumerate(ordered)}

    @lru_cache(None)
    def depths(key):
        parents = records[key][2]
        if not parents:
            return 0, 0
        values = [depths(p) for p in parents]
        return 1 + min(d[0] for d in values), 1 + max(d[1] for d in values)

    content = bytearray(b"FKWN0001")
    content += struct.pack("<I", len(ordered))
    for key in ordered:
        ancestry, queue = {}, deque([(key, 0)])
        while queue:
            ancestor, distance = queue.popleft()
            if ancestor in ancestry:
                continue
            ancestry[ancestor] = distance
            queue.extend((p, distance + 1) for p in records[ancestor][2])
        minimum, maximum = depths(key)
        content += struct.pack("<BBBH", records[key][1] == "n", minimum, maximum, len(ancestry))
        for ancestor, distance in sorted(ancestry.items(), key=lambda pair: ids[pair[0]]):
            content += struct.pack("<IB", ids[ancestor], distance)

    content += struct.pack("<I", len(index))
    for lemma in sorted(index):
        synsets = set()
        for pos in POS.values():
            forms = exceptions[pos].get(lemma)
            if forms is None:
                forms = [lemma[:-len(old)] + new for old, new in RULES[pos] if lemma.endswith(old)]
            for form in [lemma] + forms:
                for offset in index.get(form, {}).get(pos, []):
                    synsets.add(ids[pos, offset])
        word = lemma.encode()
        content += struct.pack("<H", len(word)) + word + struct.pack("<H", len(synsets))
        for synset in sorted(synsets):
            content += struct.pack("<I", synset)
    packed = gzip.compress(content, compresslevel=9, mtime=0)
    # Python versions have historically differed in the gzip OS header byte.
    packed = packed[:9] + b"\xff" + packed[10:]
    (ROOT / "assets/wordnet.bin.gz").write_bytes(packed)
    print(f"{len(records)} synsets, {len(index)} lemmas, {len(packed)} bytes")
    print("Asset SHA256:", hashlib.sha256(packed).hexdigest())


if __name__ == "__main__":
    archive = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / ".upstream/data/wordnet.zip"
    if not archive.exists():
        archive.parent.mkdir(parents=True, exist_ok=True)
        urllib.request.urlretrieve(URL, archive)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
        raise SystemExit("Unexpected WordNet archive checksum")
    build(archive)
