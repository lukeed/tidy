/** Remember the choice, so the inline script in `<head>` can apply it before paint. */
function theme() {
	let root = document.documentElement;
	let next = root.dataset.theme === 'dark' ? 'light' : 'dark';
	root.dataset.theme = next;
	localStorage.setItem('theme', next);
}

/** Copy, then say so on the button that was pressed. */
async function copy(button: HTMLElement, text: string) {
	try {
		await navigator.clipboard.writeText(text);
	} catch {
		return;
	}
	let before = button.textContent;
	button.textContent = 'copied';
	setTimeout(() => (button.textContent = before), 1400);
}

document.addEventListener('click', event => {
	let target = event.target as HTMLElement;

	let toggle = target.closest<HTMLElement>('[data-theme-toggle]');
	if (toggle) return theme();

	let copier = target.closest<HTMLElement>('[data-copy]');
	if (copier) return void copy(copier, copier.dataset.copy!);

	let block = target.closest<HTMLElement>('[data-copy-code]');
	if (block) {
		let code = block.closest('figure')?.querySelector('code');
		if (code) return void copy(block, code.textContent ?? '');
	}

	let menu = target.closest<HTMLElement>('[data-menu]');
	if (menu) {
		let open = menu.getAttribute('aria-expanded') === 'true';
		menu.setAttribute('aria-expanded', String(!open));
		document.getElementById('side')?.classList.toggle('open', !open);
		return;
	}

	if (target.closest('[data-search]')) return void search();
});

// ───────────────────────────── on this page ─────────────────────────────

/** Mark the heading the reader is in, so the contents list follows the page. */
function spy() {
	let links = [...document.querySelectorAll<HTMLAnchorElement>('.toc a')];
	if (!links.length) return;

	let byId = new Map(links.map(link => [decodeURIComponent(link.hash.slice(1)), link]));
	let seen = new Set<string>();

	let observer = new IntersectionObserver(
		entries => {
			for (let entry of entries) {
				if (entry.isIntersecting) seen.add(entry.target.id);
				else seen.delete(entry.target.id);
			}
			let current = [...byId.keys()].find(id => seen.has(id));
			for (let [id, link] of byId) link.toggleAttribute('aria-current', id === current);
		},
		{ rootMargin: '-80px 0px -70% 0px' },
	);

	for (let id of byId.keys()) {
		let heading = document.getElementById(id);
		if (heading) observer.observe(heading);
	}
}

// ───────────────────────────── search ─────────────────────────────

type Entry = { title: string; path: string; group: string; summary: string; headings: [string, string][] };

let index: Entry[] | undefined;

async function search() {
	let dialog = document.getElementById('search') as HTMLDialogElement | null;
	if (!dialog) return;

	index ??= await fetch('/search.json').then(response => response.json() as Promise<Entry[]>);

	let input = dialog.querySelector<HTMLInputElement>('[data-search-input]')!;
	let results = dialog.querySelector<HTMLElement>('[data-search-results]')!;

	function draw() {
		let query = input.value.trim().toLowerCase();
		let hits = !query
			? index!.slice(0, 8)
			: index!.filter(entry =>
					entry.title.toLowerCase().includes(query) ||
					entry.summary.toLowerCase().includes(query) ||
					entry.headings.some(([, text]) => text.toLowerCase().includes(query)),
				).slice(0, 12);

		results.innerHTML = hits.length
			? hits.map(hit => `<a href="${hit.path}"><em>${hit.group}</em><b>${hit.title}</b><span>${hit.summary}</span></a>`).join('')
			: '<p class="empty">Nothing matches that.</p>';
	}

	input.oninput = draw;
	draw();
	dialog.showModal();
	input.focus();
	input.select();
}

document.addEventListener('keydown', event => {
	if ((event.metaKey || event.ctrlKey) && event.key === 'k') {
		event.preventDefault();
		void search();
	}
});

spy();
document.addEventListener('bongo:navigate', spy as EventListener);
