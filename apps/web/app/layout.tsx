import type { Metadata } from 'next';
import './globals.css';

export const metadata: Metadata = {
  title: 'Coqui',
  description: 'A modern SEO crawler and audit workspace',
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
