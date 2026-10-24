/** A viewer preference remembered in local storage. Storage can be unavailable (private windows,
 * blocked site data); the preference then still works for this view. */
export class StoredFlag {
	value: boolean = $state(false);

	constructor(
		private readonly key: string,
		/** Stored values for on and off. */
		private readonly words: [string, string],
		fallback: boolean
	) {
		let stored: string | null = null;
		try {
			stored = localStorage.getItem(key);
		} catch {
			// Unavailable storage leaves the default.
		}
		this.value = stored === words[0] ? true : stored === words[1] ? false : fallback;
	}

	set(on: boolean) {
		this.value = on;
		try {
			localStorage.setItem(this.key, this.words[on ? 0 : 1]);
		} catch {
			// The preference still applies to this view.
		}
	}
}

/** One of several hover targets whose card opens on hover and can be pinned open by a click:
 * shared routes and stacked buses. Leaving starts a short grace period so the pointer can reach
 * the card; while the card itself is hovered it stays open. */
export class HoverPin {
	hovered: number | null = $state(null);
	pinned: number | null = $state(null);
	private cardHovered = false;
	private timer: ReturnType<typeof setTimeout> | null = null;

	/** The target whose card shows: pinned, else hovered. */
	get active(): number | null {
		return this.pinned ?? this.hovered;
	}

	open(g: number) {
		this.cancelLeave();
		if (this.hovered !== g) this.hovered = g;
	}

	leave() {
		if (this.hovered === null || this.timer || this.cardHovered) return;
		this.timer = setTimeout(() => {
			this.timer = null;
			if (!this.cardHovered) this.hovered = null;
		}, 350);
	}

	enterCard() {
		this.cardHovered = true;
		this.cancelLeave();
	}

	leaveCard() {
		this.cardHovered = false;
		this.leave();
	}

	close() {
		this.pinned = null;
		this.hovered = null;
	}

	dispose() {
		this.cancelLeave();
	}

	private cancelLeave() {
		if (this.timer) clearTimeout(this.timer);
		this.timer = null;
	}
}
