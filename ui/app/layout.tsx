import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";

import "./globals.css";

const geistSans = Geist({ variable: "--font-geist-sans", subsets: ["latin"] });
const geistMono = Geist_Mono({ variable: "--font-geist-mono", subsets: ["latin"] });

export const metadata: Metadata = {
  title: "Uptime monitor",
  description: "A Rust API and a Vercel Daemon worker that monitor HTTP endpoints.",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className={`${geistSans.variable} ${geistMono.variable}`}>
      <body className="vbg-report">
        <link rel="stylesheet" href="https://vercel.com/geist/vercel-brand.css" precedence="vbg" />
        <div className="vbg-shell">
          <a className="vbg-skip-link" href="#main">
            Skip to content
          </a>
          <header className="vbg-header">
            <div className="vbg-masthead">
              <span className="vbg-identity">
                <span className="vbg-wordmark" role="img" aria-label="Vercel" />
              </span>
              <div className="vbg-document-meta">
                <span className="vbg-context">Uptime monitor</span>
                <span className="vbg-state">
                  <span className="vbg-mono">{process.env.VERCEL_ENV ?? "local"}</span>
                </span>
              </div>
            </div>
          </header>
          <main id="main">{children}</main>
          <footer className="vbg-footer">
            <span className="vbg-logo" role="img" aria-label="Vercel" />
          </footer>
        </div>
      </body>
    </html>
  );
}
