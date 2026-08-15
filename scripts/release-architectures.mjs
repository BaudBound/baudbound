// The one place a release architecture is declared.
//
// Every verifier reads this table instead of carrying its own copy of the
// filenames, so adding an architecture is an entry here rather than an edit
// spread across the checksum, asset, package, and install checks.
//
// Debian, RPM, and the AppImage each spell the same architecture differently,
// which is why an entry carries one token per format rather than a single name.

export const WINDOWS_UPDATER_KEY = "windows-x86_64";

export const RELEASE_ARCHITECTURES = Object.freeze([
	Object.freeze({
		id: "x86_64",
		debian: "amd64",
		rpm: "x86_64",
		appImage: "amd64",
		updaterKey: "linux-x86_64",
	}),
	Object.freeze({
		id: "aarch64",
		debian: "arm64",
		rpm: "aarch64",
		appImage: "aarch64",
		updaterKey: "linux-aarch64",
	}),
]);

export function debianPackageName(version, architecture) {
	return `Baudbound_${version}_${architecture.debian}.deb`;
}

export function rpmPackageName(version, architecture) {
	return `Baudbound-${version}-1.${architecture.rpm}.rpm`;
}

export function expectedLinuxArtifacts(version) {
	return RELEASE_ARCHITECTURES.flatMap((architecture) => {
		const artifacts = [
			{
				label: `Linux Debian package (${architecture.id})`,
				format: "deb",
				architecture,
				name: debianPackageName(version, architecture),
				matches: (name) => name === debianPackageName(version, architecture),
			},
			{
				label: `Linux RPM package (${architecture.id})`,
				format: "rpm",
				architecture,
				name: rpmPackageName(version, architecture),
				matches: (name) => name === rpmPackageName(version, architecture),
			},
		];

		// An architecture without an AppImage has no updater payload either, so
		// omitting it here is what removes it from the whole release contract.
		if (architecture.appImage) {
			artifacts.push({
				label: `Linux AppImage (${architecture.id})`,
				format: "appimage",
				architecture,
				name: null,
				matches: (name) => name.endsWith(`_${architecture.appImage}.AppImage`),
			});
		}

		return artifacts;
	});
}

export function expectedArtifacts(version) {
	return [
		{
			label: "Windows NSIS installer",
			format: "nsis",
			architecture: null,
			name: null,
			matches: (name) => name.endsWith("-setup.exe"),
		},
		...expectedLinuxArtifacts(version),
	];
}

export function updaterKeys() {
	return [
		WINDOWS_UPDATER_KEY,
		...RELEASE_ARCHITECTURES.filter((entry) => entry.updaterKey).map((entry) => entry.updaterKey),
	];
}
