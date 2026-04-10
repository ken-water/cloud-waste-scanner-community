export const PDF_BRAND_SITE_URL = "local-community";
export const PDF_BRAND_SITE_HREF = `https://${PDF_BRAND_SITE_URL}`;

interface PdfLinkOptions {
  url: string;
}

interface PdfDocLike {
  setFillColor: (...args: number[]) => void;
  roundedRect: (...args: any[]) => void;
  setFontSize: (size: number) => void;
  setTextColor: (...args: number[]) => void;
  text: (text: string, x: number, y: number, options?: any) => void;
  setDrawColor: (...args: number[]) => void;
  line: (x1: number, y1: number, x2: number, y2: number) => void;
  getTextWidth: (text: string) => number;
  link: (x: number, y: number, width: number, height: number, options: PdfLinkOptions) => void;
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

export function formatPdfDateTime(value: Date | number): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "-";
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

interface PdfHeaderOptions {
  title: string;
  generatedAt?: Date | number;
  extraLines?: string[];
}

export function drawPdfBrandHeader(doc: PdfDocLike, options: PdfHeaderOptions): number {
  const generatedAt = options.generatedAt ?? new Date();
  const extraLines = options.extraLines?.filter((line) => line.trim().length > 0) ?? [];

  doc.setFillColor(79, 70, 229);
  doc.roundedRect(14, 11, 11, 11, 2, 2, "F");
  doc.setFontSize(7);
  doc.setTextColor(255, 255, 255);
  doc.text("CWS", 19.5, 18, { align: "center" });

  doc.setFontSize(22);
  doc.setTextColor(40, 40, 40);
  doc.text(options.title, 29, 22);

  doc.setFontSize(10);
  doc.setTextColor(100, 100, 100);
  doc.text(`Generated: ${formatPdfDateTime(generatedAt)}`, 14, 28);

  let lastY = 28;
  for (const line of extraLines) {
    lastY += 5;
    doc.text(line, 14, lastY);
  }
  return lastY;
}

export function drawPdfFooterSiteLink(
  doc: PdfDocLike,
  pageWidth: number,
  pageHeight: number,
  pageNumber: number,
  totalPages: number
): void {
  const footerY = pageHeight - 8;
  doc.setDrawColor(225);
  doc.line(14, pageHeight - 13, pageWidth - 14, pageHeight - 13);
  doc.setFontSize(9);

  doc.setTextColor(37, 99, 235);
  doc.text(PDF_BRAND_SITE_URL, 14, footerY);
  const linkWidth = doc.getTextWidth(PDF_BRAND_SITE_URL);
  doc.link(14, footerY - 4, linkWidth, 5, { url: PDF_BRAND_SITE_HREF });

  doc.setTextColor(120);
  doc.text(`Page ${pageNumber}/${totalPages}`, pageWidth - 14, footerY, { align: "right" });
}
