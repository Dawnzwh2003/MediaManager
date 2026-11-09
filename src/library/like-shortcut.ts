export function isLikeShortcut(event:Pick<KeyboardEvent,"key"|"repeat"|"ctrlKey"|"altKey"|"metaKey">):boolean {
 return event.key.toLowerCase()==="l"&&!event.repeat&&!event.ctrlKey&&!event.altKey&&!event.metaKey;
}
