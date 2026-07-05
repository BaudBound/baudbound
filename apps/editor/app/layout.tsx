import "@/styles/globals.css";
import type { Metadata } from "next";

const appName = "BaudBound Editor";
const appDescription =
	"Build, verify, simulate, and export local-first BaudBound visual automation scripts as .bbs packages.";
const appUrl = process.env.NEXT_PUBLIC_EDITOR_URL ?? process.env.NEXT_PUBLIC_SITE_URL ?? "https://editor.baudbound.app";

export const metadata: Metadata = {
	metadataBase: new URL(appUrl),
	applicationName: appName,
	title: {
		default: appName,
		template: `%s | ${appName}`,
	},
	description: appDescription,
	keywords: [
		"BaudBound",
		"BaudBound Editor",
		"visual scripting",
		"automation",
		"local-first automation",
		"script editor",
		".bbs",
	],
	authors: [{ name: "BaudBound" }],
	creator: "BaudBound",
	publisher: "BaudBound",
	alternates: {
		canonical: "/",
	},
	icons: {
		icon: [
			{ url: "/icon_x16.ico", sizes: "16x16", type: "image/x-icon" },
			{ url: "/icon_x32.ico", sizes: "32x32", type: "image/x-icon" },
			{ url: "/icon_x48.ico", sizes: "48x48", type: "image/x-icon" },
			{ url: "/icon_x64.ico", sizes: "64x64", type: "image/x-icon" },
			{ url: "/icon_x128.ico", sizes: "128x128", type: "image/x-icon" },
			{ url: "/icon_x256.ico", sizes: "256x256", type: "image/x-icon" },
		],
		shortcut: [{ url: "/icon_x32.ico" }],
	},
	openGraph: {
		type: "website",
		url: "/",
		siteName: "BaudBound",
		title: appName,
		description: appDescription,
		images: [
			{
				url: "/logo.svg",
				width: 800,
				height: 800,
				alt: "BaudBound logo",
				type: "image/svg+xml",
			},
		],
	},
	twitter: {
		card: "summary",
		title: appName,
		description: appDescription,
		images: ["/logo.svg"],
	},
	robots: {
		index: true,
		follow: true,
	},
};

export default function RootLayout({
	children,
}: Readonly<{
	children: React.ReactNode;
}>) {
	return (
		<html lang="en" className="h-full antialiased">
			<body className="min-h-full flex flex-col">{children}</body>
		</html>
	);
}
