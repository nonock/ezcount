<script lang="ts" module>
	import { type VariantProps, tv } from "tailwind-variants/lite";
	import { cn, type WithElementRef } from "@/lib/utils.js";
	import type { HTMLAnchorAttributes, HTMLButtonAttributes } from "svelte/elements";

	// daisyUI's button: semibold, 2.5rem tall by default, and raised (a light top edge and a soft
	// shadow in the button's own color, --btn-depth) until pressed.
	const depth =
		"shadow-[inset_0_0.5px_0_0.5px_oklch(100%_0_0/6%),0_3px_2px_-2px_color-mix(in_oklab,var(--btn-depth)_30%,transparent),0_4px_3px_-2px_color-mix(in_oklab,var(--btn-depth)_30%,transparent)] active:shadow-none";

	export const buttonVariants = tv({
		base: "focus-visible:border-ring focus-visible:ring-ring/50 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive dark:aria-invalid:border-destructive/50 rounded-lg border border-transparent bg-clip-padding text-sm font-semibold focus-visible:ring-3 aria-invalid:ring-3 active:not-aria-[haspopup]:translate-y-px [&_svg:not([class*='size-'])]:size-4 group/button inline-flex shrink-0 items-center justify-center whitespace-nowrap transition-all outline-none select-none disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		variants: {
			variant: {
				default: `bg-primary text-primary-foreground border-[color-mix(in_oklab,var(--primary),black_5%)] hover:bg-[color-mix(in_oklab,var(--primary),black_7%)] [--btn-depth:var(--primary)] ${depth}`,
				// Flat and bordered: a quieter step below the filled grey `secondary`.
				outline: "bg-card text-foreground border-input hover:bg-accent aria-expanded:bg-accent",
				secondary: `bg-secondary text-secondary-foreground border-[color-mix(in_oklab,var(--secondary),black_5%)] hover:bg-[color-mix(in_oklab,var(--secondary),black_7%)] aria-expanded:bg-[color-mix(in_oklab,var(--secondary),black_7%)] [--btn-depth:var(--secondary)] ${depth}`,
				ghost: "hover:bg-muted hover:text-foreground aria-expanded:bg-muted aria-expanded:text-foreground",
				destructive: "bg-destructive-soft hover:bg-[color-mix(in_oklab,var(--destructive-soft),var(--destructive)_12%)] focus-visible:ring-destructive/20 dark:focus-visible:ring-destructive/40 text-destructive focus-visible:border-destructive/40",
				link: "text-link underline-offset-4 hover:underline",
			},
			size: {
				default: "h-10 gap-1.5 px-4 has-data-[icon=inline-end]:pr-3 has-data-[icon=inline-start]:pl-3",
				xs: "h-6 gap-1 px-2 text-xs has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3",
				sm: "h-8 gap-1 px-3 text-xs has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2 [&_svg:not([class*='size-'])]:size-3.5",
				lg: "h-12 gap-2 px-5 text-base has-data-[icon=inline-end]:pr-4 has-data-[icon=inline-start]:pl-4 [&_svg:not([class*='size-'])]:size-5",
				icon: "size-10",
				"icon-xs": "size-6 [&_svg:not([class*='size-'])]:size-3",
				"icon-sm": "size-8",
				"icon-lg": "size-12 [&_svg:not([class*='size-'])]:size-5",
			},
		},
		defaultVariants: {
			variant: "default",
			size: "default",
		},
	});

	export type ButtonVariant = VariantProps<typeof buttonVariants>["variant"];
	export type ButtonSize = VariantProps<typeof buttonVariants>["size"];

	export type ButtonProps = WithElementRef<HTMLButtonAttributes> &
		WithElementRef<HTMLAnchorAttributes> & {
			variant?: ButtonVariant;
			size?: ButtonSize;
		};
</script>

<script lang="ts">
	let {
		class: className,
		variant = "default",
		size = "default",
		ref = $bindable(null),
		href = undefined,
		type = "button",
		disabled,
		children,
		...restProps
	}: ButtonProps = $props();
</script>

{#if href}
	<a
		bind:this={ref}
		data-slot="button"
		class={cn(buttonVariants({ variant, size }), className)}
		href={disabled ? undefined : href}
		aria-disabled={disabled}
		role={disabled ? "link" : undefined}
		tabindex={disabled ? -1 : undefined}
		{...restProps}
	>
		{@render children?.()}
	</a>
{:else}
	<button
		bind:this={ref}
		data-slot="button"
		class={cn(buttonVariants({ variant, size }), className)}
		{type}
		{disabled}
		{...restProps}
	>
		{@render children?.()}
	</button>
{/if}
