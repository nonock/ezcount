/** Presses the button of a toast, given the options it was shown with. */
export function pressAction(options: unknown) {
  (options as { action: { onClick: () => void } }).action.onClick();
}
