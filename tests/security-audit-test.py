#!/usr/bin/env python3
import datetime
import importlib.util
import json
import pathlib
import tempfile
import unittest

SOURCE = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("audit", SOURCE / "scripts/security-audit.py")
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class PolicyTest(unittest.TestCase):
    def test_unknown_advisory_blocks_the_actual_gate(self):
        import contextlib
        import io
        from unittest.mock import patch
        def response(endpoint, data=None):
            if endpoint == "querybatch":
                return {"results": [{"vulns": [{"id": "NEW-UNREVIEWED"}]}]}
            return {"id": "NEW-UNREVIEWED", "summary": "fixture only"}
        with patch("sys.argv", ["security-audit.py"]), patch.object(audit.subprocess, "check_output", return_value="fixture v1.0.0\n"), patch.object(audit, "osv", side_effect=response), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(audit.main(), 1)

    def test_enabling_http_invalidates_stdio_exception(self):
        from unittest.mock import patch
        def graph(command, **kwargs):
            return 'rmcp feature "transport-streamable-http-server"\n' if "--invert" in command else "rmcp v0.8.5\n"
        with patch("sys.argv", ["security-audit.py"]), patch.object(audit.subprocess, "check_output", side_effect=graph):
            with self.assertRaisesRegex(ValueError, "HTTP/OAuth/client enabled"):
                audit.main()

    def test_scoped_exceptions_do_not_allow_new_packages_versions_or_advisories(self):
        entries = audit.exceptions(SOURCE / "security/advisory-exceptions.json")
        old = {"id": "RUSTSEC-2026-0189"}
        self.assertTrue(audit.classify("rmcp", "0.8.5", "app-arm64", old, entries))
        for package, version, scope, advisory in [
            ("russh", "0.58.0", "app-arm64", old),
            ("rmcp", "0.9.0", "app-arm64", old),
            ("rmcp", "0.8.5", "new-target", old),
            ("rmcp", "0.8.5", "app-arm64", {"id": "NEW-UNREVIEWED"}),
        ]:
            self.assertIsNone(audit.classify(package, version, scope, advisory, entries))

    def test_expired_and_incomplete_exceptions_fail(self):
        entries = json.loads((SOURCE / "security/advisory-exceptions.json").read_text())
        with tempfile.TemporaryDirectory() as directory:
            file = pathlib.Path(directory) / "exceptions.json"
            entries[0]["expires"] = (datetime.date.today() - datetime.timedelta(days=1)).isoformat()
            file.write_text(json.dumps(entries))
            with self.assertRaisesRegex(ValueError, "Expired"):
                audit.exceptions(file)
            file.write_text('[{"package":"rmcp"}]')
            with self.assertRaisesRegex(ValueError, "Incomplete"):
                audit.exceptions(file)

    def test_network_failure_is_not_reported_as_no_vulnerabilities(self):
        from unittest.mock import patch
        with patch("urllib.request.urlopen", side_effect=audit.urllib.error.URLError("offline")), patch("time.sleep"):
            with self.assertRaises(audit.urllib.error.URLError):
                audit.osv("querybatch", {"queries": []})


if __name__ == "__main__":
    unittest.main()
