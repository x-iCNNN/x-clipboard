"""Fetch the PDFium ABI matched to pdfium-render 0.9.4, verifying its SHA-256."""
import argparse
import hashlib
import io
from pathlib import Path, PurePosixPath
import tarfile
import urllib.request

HASHES = {
    "win-x64": "73cc0de638ac2095e7445bf56a38200a5b7c7ca0e9f4ba144598f2457377ac08",
    "mac-x64": "6dedf83990e0e3d6b7c93c9e7589c5a126b0ae14b7464d76120cff7a26afb18b",
    "mac-arm64": "52e94ca5aa8847934330daf3f8150c190682c5ca93831468794f8b90d4392e40",
    "linux-x64": "1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d",
}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=HASHES, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--archive", type=Path, help="Use an already downloaded archive; checksum is still required")
    args = parser.parse_args()
    url = f"https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/7881/pdfium-{args.platform}.tgz"
    if args.archive:
        archive = args.archive.read_bytes()
    else:
        with urllib.request.urlopen(url, timeout=90) as response:
            archive = response.read(64 * 1024 * 1024)
    if hashlib.sha256(archive).hexdigest() != HASHES[args.platform]:
        raise RuntimeError("PDFium archive checksum mismatch; nothing installed")
    args.output.mkdir(parents=True, exist_ok=True)
    found = False
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tar:
        for member in tar:
            path = PurePosixPath(member.name)
            if not member.isfile() or path.is_absolute() or ".." in path.parts:
                continue
            if path.name in ("pdfium.dll", "libpdfium.dylib", "libpdfium.so"):
                target = args.output / path.name
                found = True
            elif "license" in member.name.lower():
                target = args.output / "licenses" / "pdfium" / Path(*path.parts)
            else:
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            with tar.extractfile(member) as source:
                target.write_bytes(source.read())
    if not found:
        raise RuntimeError("Verified archive did not contain a runtime library")
    print(f"PDFium chromium/7881 installed in {args.output.resolve()} (SHA-256 verified)")

if __name__ == "__main__":
    main()
