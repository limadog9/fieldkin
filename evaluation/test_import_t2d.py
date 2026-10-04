"""Importer safety and label-preservation tests; never executes a matcher."""

import io
import tarfile
import unittest

import import_t2d


class AnnotationImportTests(unittest.TestCase):
    def test_distinct_ids_duplicate_names_and_multiple_positive_labels(self):
        data = b'"urn:first","Same Name","True","2"\r\n"urn:second","Same Name","","2"\r\n"urn:first","Same Name","False","5"\r\n'
        fields = import_t2d.parse_attributes(data)
        self.assertEqual([field["index"] for field in fields], [2, 5])
        self.assertEqual(fields[0]["name"], "Same Name")
        self.assertEqual(fields[0]["positive_targets"], ["urn:first", "urn:second"])
        self.assertEqual(fields[1]["positive_targets"], ["urn:first"])

    def test_empty_header_is_preserved_and_conflicting_header_rejected(self):
        self.assertEqual(import_t2d.parse_attributes(b'"urn:label","","","0"\n')[0]["name"], "")
        with self.assertRaises(ValueError):
            import_t2d.parse_attributes(b'"urn:a","A","","0"\n"urn:b","B","","0"\n')

    def test_archive_paths_and_links_are_rejected_without_extraction(self):
        for name, kind in [("../1.csv", tarfile.REGTYPE), ("1.csv", tarfile.SYMTYPE)]:
            stream = io.BytesIO()
            with tarfile.open(fileobj=stream, mode="w:gz") as archive:
                member = tarfile.TarInfo(name)
                member.type = kind
                archive.addfile(member, io.BytesIO())
            with self.assertRaises(ValueError):
                import_t2d.convert(stream.getvalue(), b"")

    def test_duplicate_class_and_malformed_column_index_are_rejected(self):
        row = b'"1.tar.gz","Class","urn:class","0"\n'
        with self.assertRaises(ValueError):
            import_t2d.parse_classes(row + row)
        with self.assertRaises(ValueError):
            import_t2d.parse_attributes(b'"urn:a","Name","True","-1"\n')

    def test_offline_fixture_reproduces_exactly(self):
        sources = import_t2d.load_sources()
        fixture = import_t2d.convert(sources["attributes_complete.tar.gz"], sources["classes_complete.csv"])
        expected = (import_t2d.DIRECTORY / "corpus.json").read_bytes().replace(b"\r\n", b"\n")
        self.assertEqual(import_t2d.encoded(fixture), expected)


if __name__ == "__main__":
    unittest.main()
