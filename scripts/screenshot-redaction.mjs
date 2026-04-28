const SECRET_PATTERNS = [
  [/([?&](?:access_token|refresh_token|client_secret|id_token|code)=)[^&\s"']+/gi, "$1<redacted>"],
  [
    /("(?:access_token|refresh_token|client_secret|id_token|email|aud|azp|sub|authorization|cookie|set-cookie)"\s*:\s*")[^"]+(")/gi,
    "$1<redacted>$2",
  ],
  [/\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi, "<email:redacted>"],
  [/\b(?:Bearer\s+)?(?:ya29|1\/\/|GOCSPX-)[A-Za-z0-9._~+/=-]{8,}\b/g, "<token:redacted>"],
  [/\b(?:sk-[A-Za-z0-9_-]{20,}|sk-ant-[A-Za-z0-9_-]{20,}|github_pat_[A-Za-z0-9_]{20,})\b/g, "<token:redacted>"],
  [/\b[A-Za-z0-9_-]{24,}\.[A-Za-z0-9_-]{16,}\.[A-Za-z0-9_-]{16,}\b/g, "<jwt:redacted>"],
  [/\b(__cf[^=;\s]*|_cfuvid|cf_clearance)=[^;\s]+/gi, "$1=<redacted>"],
];

export function redactText(value) {
  return SECRET_PATTERNS.reduce((text, [pattern, replacement]) => text.replace(pattern, replacement), value);
}

export async function redactVisibleText(page, selectors) {
  await page.evaluate(
    ({ selectors: pageSelectors, patternSources }) => {
      const patterns = patternSources.map(([source, flags, replacement]) => [new RegExp(source, flags), replacement]);
      const redact = (value) => patterns.reduce((text, [pattern, replacement]) => text.replace(pattern, replacement), value);

      document.querySelectorAll(pageSelectors).forEach((element) => {
        const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
        const textNodes = [];
        while (walker.nextNode()) textNodes.push(walker.currentNode);

        textNodes.forEach((node) => {
          const original = node.nodeValue ?? "";
          const redacted = redact(original);
          if (redacted !== original) node.nodeValue = redacted;
        });

        if (element instanceof HTMLElement && element.dataset.copy) {
          element.dataset.copy = redact(element.dataset.copy);
        }
      });
    },
    {
      selectors,
      patternSources: SECRET_PATTERNS.map(([pattern, replacement]) => [pattern.source, pattern.flags, replacement]),
    },
  );
}
