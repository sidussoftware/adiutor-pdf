"""Writes tests/fixtures/encrypted.pdf: 2 blank pages (200x300 and 300x200 points),
encrypted with user password "secret" (AES-128, which PDFium can open)."""

from pathlib import Path

from pypdf import PdfWriter

writer = PdfWriter()
writer.add_blank_page(width=200, height=300)
writer.add_blank_page(width=300, height=200)
writer.encrypt(user_password="secret", algorithm="AES-128")
out = Path(__file__).with_name("encrypted.pdf")
with out.open("wb") as f:
    writer.write(f)
