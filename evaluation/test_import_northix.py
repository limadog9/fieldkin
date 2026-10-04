"""Offline importer checks. These tests never execute a schema matcher."""

import io
import json
import stat
import unittest
from unittest.mock import patch
import warnings
import zipfile

import import_northix as northix


def archive(entries):
    stream = io.BytesIO()
    with zipfile.ZipFile(stream, "w", compression=zipfile.ZIP_DEFLATED) as output:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            for name, content in entries:
                if isinstance(name, str):
                    entry = zipfile.ZipInfo("placeholder")
                    entry.filename = name  # Preserve hostile separators on Windows too.
                    entry.compress_type = zipfile.ZIP_DEFLATED
                else:
                    entry = name
                output.writestr(entry, content)
    return stream.getvalue()


def small_entries():
    return [("Northix/InputData/a@left@1.dat", b"first\r\n\r\nthird\r\n"),
            ("Northix/Classes/shared/a@left@1.dat", b"different label copy\n"),
            ("Northix/InputData/b@right@2.txt", b"first\nsecond\n"),
            ("Northix/Classes/shared/b@right@2.txt", b"first\nsecond\n")]


class ImportNorthixTests(unittest.TestCase):
    def test_pinned_archive_reproduces_fixture_and_label_inventory(self):
        value = northix.convert(northix.load_source())
        self.assertEqual(northix.encoded(value), northix.FIXTURE.read_bytes())
        counts = northix.inventory(value)
        self.assertEqual(counts["table_pairs"], 84)
        self.assertEqual(counts["unique_fields"], 115)
        self.assertEqual(counts["positive_edges"], 28)
        self.assertEqual(counts["source_field_occurrences"], 469)
        self.assertEqual(counts["no_match_field_occurrences"], 441)
        self.assertEqual(counts["explicit_unclassed_field_occurrences"], 70)
        self.assertEqual(counts["value_copy_discrepancies"], 15)
        self.assertEqual(value["encoding_audit"]["non_ascii_files"], 11)
        self.assertEqual(len({label["field"] for label in value["labels"]}), 115)

    def test_evenly_spaced_samples_are_bounded_unique_and_include_endpoints(self):
        for size in [0, 1, 2, 29, 63, 64, 65, 100, 16049]:
            indices = northix.sample_indices(size)
            self.assertEqual(len(indices), min(64, size))
            self.assertEqual(indices, sorted(set(indices)))
            if size:
                self.assertEqual(indices[0], 0)
                self.assertEqual(indices[-1], size - 1)
        self.assertEqual(northix.sample_indices(100, 4), [0, 33, 66, 99])
        for size, cap in [(-1, 64), (2, 0)]:
            with self.assertRaises(ValueError):
                northix.sample_indices(size, cap)

    def test_class_values_are_never_sample_inputs(self):
        value = northix.convert(archive(small_entries()))
        field = value["tables"][0]["fields"][0]
        self.assertEqual(field["samples"], ["first", None, "third"])
        self.assertEqual(field["name"], "a")
        self.assertNotIn("class", field)
        self.assertEqual(value["discrepancies"][0]["field"], field["id"])
        self.assertEqual(value["pairs"], [{"id": "db1:left->db2:right", "source_table": "db1:left", "target_table": "db2:right"}])

    def test_unclassed_is_not_a_positive_equivalence_class(self):
        entries = [(name.replace("/shared/", "/UNCLASSED/"), content) for name, content in small_entries()]
        counts = northix.inventory(northix.convert(archive(entries)))
        self.assertEqual(counts["positive_edges"], 0)
        self.assertEqual(counts["no_match_field_occurrences"], 1)
        self.assertEqual(counts["explicit_unclassed_field_occurrences"], 1)

    def test_missing_unknown_and_duplicate_class_annotations_are_rejected(self):
        entries = small_entries()
        variants = [entries[:-1], entries + [("Northix/Classes/other/a@left@1.dat", b"x")],
                    entries + [("Northix/Classes/shared/unknown@left@1.dat", b"x")]]
        for variant in variants:
            with self.assertRaises(ValueError):
                northix.convert(archive(variant))

    def test_unsafe_duplicate_symlink_and_unsupported_members_are_rejected(self):
        for name in ["../outside", "/Northix/ReadMe.txt", "Northix/../ReadMe.txt", "Northix\\ReadMe.txt", "Northix/C:evil", "Northix/unexpected.txt"]:
            with self.assertRaises(ValueError):
                northix.read_members(archive([(name, b"x")]))
        with self.assertRaises(ValueError):
            northix.read_members(archive([("Northix/ReadMe.txt", b"x"), ("Northix/ReadMe.txt", b"y")]))
        with self.assertRaises(ValueError):
            northix.read_members(archive(small_entries() + [("Northix//InputData/a@left@1.dat", b"shadow")]))
        link = zipfile.ZipInfo("Northix/ReadMe.txt")
        link.create_system = 3
        link.external_attr = (stat.S_IFLNK | 0o777) << 16
        with self.assertRaises(ValueError):
            northix.read_members(archive([(link, b"target")]))

    def test_archive_count_and_expansion_budgets_are_enforced(self):
        sample = archive(small_entries())
        for name, limit in [("MAX_MEMBERS", 3), ("MAX_MEMBER_BYTES", 3), ("MAX_EXPANDED_BYTES", 3)]:
            with patch.object(northix, name, limit), self.assertRaises(ValueError):
                northix.read_members(sample)
        with self.assertRaises(ValueError):
            northix.read_members(b"x" * (northix.SOURCE["bytes"] + 1))

    def test_ambiguous_control_bytes_are_rejected_without_encoding_guessing(self):
        entries = small_entries()
        for bad in [b"\x80", b"\x00"]:
            entries[0] = (entries[0][0], bad)
            with self.assertRaises(ValueError):
                northix.convert(archive(entries))

    def test_manifest_fixture_hash_and_canonical_utf8_encoding_agree(self):
        provenance = json.loads((northix.DIRECTORY / "provenance.json").read_text("utf-8"))
        self.assertEqual(provenance["derived_fixture"]["sha256"], northix.digest(northix.FIXTURE.read_bytes()))
        self.assertNotIn(b"\r\n", northix.FIXTURE.read_bytes())
        self.assertEqual(provenance["archive"], northix.SOURCE)


if __name__ == "__main__":
    unittest.main()
