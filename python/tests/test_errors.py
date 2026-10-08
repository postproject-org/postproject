from __future__ import annotations

import ctypes
import unittest
from unittest import mock

from postproject import (
    AmbiguousResolutionError,
    FingerprintError,
    InternalError,
    IoError,
    MigrationError,
    UnsupportedError,
    _abi,
    _native,
    _production,
)
from postproject._errors import ERROR_TYPES


class ErrorMappingTests(unittest.TestCase):
    def test_required_unknown_native_tags_are_unsupported(self) -> None:
        decoders = (
            _production._representation_kind,
            _production._job_state,
            _production._dependency_set_status,
            _production._content_structure_kind,
            _production._locator_availability,
            _production._representation_availability,
            _production._evidence_kind,
        )
        for decoder in decoders:
            with self.subTest(decoder=decoder.__name__):
                with self.assertRaises(UnsupportedError) as caught:
                    decoder(999)
                self.assertEqual(caught.exception.code, _abi.PP_ERROR_UNSUPPORTED)
        reference = _abi.ObjectRef()
        reference.kind = 999
        with self.assertRaises(UnsupportedError):
            _production._object_reference(reference)

    def test_unknown_conflict_detail_still_releases_native_error(self) -> None:
        library = mock.Mock()
        library.pp_error_message.return_value = b"conflict"

        def conflict_detail(error: object, output: ctypes.c_void_p) -> bool:
            pointer = ctypes.cast(output, ctypes.POINTER(_abi.TransactionConflict))
            pointer.contents.kind = 999
            return True

        library.pp_error_transaction_conflict.side_effect = conflict_detail
        owner = object.__new__(_native.NativeLibrary)
        owner.lib = library
        error = ctypes.pointer(_abi.Error())
        with self.assertRaises(UnsupportedError):
            owner.check(_abi.PP_ERROR_CONFLICT, error)
        library.pp_error_release.assert_called_once_with(error)

    def test_specialized_native_failures_have_public_exception_types(self) -> None:
        self.assertIs(ERROR_TYPES[4], IoError)
        self.assertIs(ERROR_TYPES[6], MigrationError)
        self.assertIs(ERROR_TYPES[8], AmbiguousResolutionError)
        self.assertIs(ERROR_TYPES[9], FingerprintError)
        self.assertIs(ERROR_TYPES[255], InternalError)


if __name__ == "__main__":
    unittest.main()
