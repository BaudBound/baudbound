import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import Script from "next/script";

import "../styles/globals.css";

// Read fresh per request so compose.yaml env vars take effect without rebuilding the image.
export const dynamic = "force-dynamic";

const inter = Inter({
	subsets: ["latin"],
	variable: "--font-inter",
	weight: ["400", "500", "600", "700", "800"],
});

const jetbrainsMono = JetBrains_Mono({
	subsets: ["latin"],
	variable: "--font-jetbrains-mono",
	weight: ["400", "500", "600", "700"],
});

export const metadata: Metadata = {
	title: "BaudBound: Automate your machine, visually.",
	description:
		"Build a workflow by connecting nodes in the browser editor, export it as a portable package, and run it natively on your own machine. No cloud. No account.",
	icons: {
		icon: [
			{ url: "/icon_x16.ico", sizes: "16x16" },
			{ url: "/icon_x32.ico", sizes: "32x32" },
			{ url: "/icon_x48.ico", sizes: "48x48" },
		],
	},
};

export default function RootLayout({
	children,
}: Readonly<{
	children: React.ReactNode;
}>) {
	const umamiSrc = process.env.UMAMI_SRC;
	const umamiWebsiteId = process.env.UMAMI_WEBSITE_ID;

	return (
		<html lang="en" className={`${inter.variable} ${jetbrainsMono.variable}`}>
			<body className="font-sans antialiased">
				{children}
				{umamiSrc && umamiWebsiteId && (
					<Script src={umamiSrc} data-website-id={umamiWebsiteId} strategy="afterInteractive" />
				)}
			</body>
		</html>
	);
}
