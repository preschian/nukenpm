export const SITE = {
	url: 'https://nukenpm.avalix.dev',
	name: 'nukenpm',
	title: 'nukenpm — Nuke the node_modules eating your disk',
	description:
		'nukenpm — a fast, keyboard-driven terminal app that finds and wipes the node_modules (and other build folders) eating your disk.',
	version: __NUKENPM_VERSION__,
	ogImage: '/og-image.jpg',
	themeColor: '#0d0e11',
	github: 'https://github.com/preschian/nukenpm',
	homebrew: 'https://github.com/preschian/homebrew-tap',
	install: 'brew install preschian/tap/nukenpm',
} as const;
