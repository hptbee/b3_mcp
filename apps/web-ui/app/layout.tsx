import "./globals.css";
import "@xyflow/react/dist/style.css";

import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "B3 Control",
  description: "Local-first control UI for the B3 MCP code intelligence platform"
};

export default function RootLayout({
  children
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
