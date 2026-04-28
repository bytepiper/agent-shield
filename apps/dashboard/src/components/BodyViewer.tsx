import { useQuery } from "@tanstack/react-query";
import { Check, Copy } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { fetchBody } from "../lib/api";
import type { EventBody } from "../lib/types";

type BodyViewerProps = {
  inlineBody?: EventBody | null;
  fileName?: string | null;
};

function inlineBodyText(body?: EventBody | null) {
  if (!body) return null;
  if (body.text) return body.text;
  if (body.base64) {
    return JSON.stringify(
      {
        encoding: "base64",
        data: body.base64,
        bytes: body.bytes,
        truncated: body.truncated,
      },
      null,
      2,
    );
  }
  return null;
}

function formatBodyContent(content: string) {
  try {
    return JSON.stringify(JSON.parse(content), null, 2);
  } catch {
    return content;
  }
}

export function BodyViewer({ inlineBody, fileName }: BodyViewerProps) {
  const [copied, setCopied] = useState(false);
  const inlineText = inlineBodyText(inlineBody);
  const bodyQuery = useQuery({
    queryKey: ["body", fileName],
    queryFn: () => fetchBody(fileName!),
    enabled: !inlineText && Boolean(fileName),
    staleTime: Infinity,
  });

  const content = useMemo(
    () => formatBodyContent(inlineText ?? bodyQuery.data ?? ""),
    [bodyQuery.data, inlineText],
  );

  useEffect(() => {
    if (!copied) return;
    const timeoutId = window.setTimeout(() => setCopied(false), 1200);
    return () => window.clearTimeout(timeoutId);
  }, [copied]);

  if (!inlineText && !fileName) {
    return <div className="text-sm text-white/45">No body captured.</div>;
  }

  if (bodyQuery.isLoading) {
    return <div className="text-sm text-white/45">Loading body…</div>;
  }

  if (bodyQuery.error instanceof Error) {
    return <div className="text-sm text-rose-300">{bodyQuery.error.message}</div>;
  }

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <button
          className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-black/30 px-3 py-1.5 text-xs text-white/70 transition hover:border-white/20 hover:text-white"
          onClick={handleCopy}
          type="button"
        >
          {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <pre className="max-h-[30rem] overflow-auto whitespace-pre-wrap break-words rounded-2xl border border-white/10 bg-black/40 p-4 text-xs leading-6 text-white/85">
        {content}
      </pre>
    </div>
  );
}
