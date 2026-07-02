export async function copyText(text: string): Promise<boolean> {
	if (navigator.clipboard?.writeText) {
		try {
			await navigator.clipboard.writeText(text);
			return true;
		} catch {
			/* fall through to legacy path */
		}
	}

	try {
		const ta = document.createElement('textarea');
		ta.value = text;
		ta.style.position = 'fixed';
		ta.style.left = '-9999px';
		document.body.appendChild(ta);
		ta.select();
		const ok = document.execCommand('copy');
		document.body.removeChild(ta);
		return ok;
	} catch {
		return false;
	}
}

export function wireCopyButtons(liveRegion: HTMLElement | null) {
	document.querySelectorAll<HTMLButtonElement>('.copy-btn').forEach((btn) => {
		let timer: ReturnType<typeof setTimeout>;
		btn.addEventListener('click', async () => {
			const text = btn.dataset.copy || '';
			const ok = await copyText(text);
			if (liveRegion) {
				liveRegion.textContent = ok ? 'Install command copied to clipboard' : 'Could not copy — select and copy manually';
			}
			btn.textContent = ok ? 'copied ✓' : 'failed';
			btn.style.background = ok ? 'rgba(95,208,197,.16)' : 'rgba(224,138,122,.16)';
			btn.style.color = ok ? '#5fd0c5' : '#e08a7a';
			clearTimeout(timer);
			timer = setTimeout(() => {
				btn.textContent = 'copy';
				btn.style.background = '#5fd0c5';
				btn.style.color = '#0d0e11';
				if (liveRegion) liveRegion.textContent = '';
			}, 1600);
		});
	});
}
