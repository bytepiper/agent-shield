export type HeaderKv = {
  name: string;
  value: string;
};

export type EventBody = {
  bytes: number;
  truncated: boolean;
  text?: string | null;
  base64?: string | null;
};

export type Alert = {
  pattern: string;
  action: string;
  matched: string;
};

export type TrafficEntry = {
  id: number;
  ts: string;
  session_id?: number | null;
  event_seq?: number;
  session_seq?: number;
  phase?: string;
  transport?: string;
  direction?: string | null;
  method?: string | null;
  url?: string | null;
  preview?: string | null;
  domain: string;
  status?: number | null;
  content_type?: string | null;
  req_bytes?: number | null;
  resp_bytes?: number | null;
  action: string;
  decision_action?: string | null;
  decision_reason?: string | null;
  duration_ms?: number | null;
  alerts?: Alert[];
  req_headers?: HeaderKv[];
  resp_headers?: HeaderKv[];
  req_body_file?: string | null;
  resp_body_file?: string | null;
  req_body?: EventBody | null;
  resp_body?: EventBody | null;
};

export type Stats = {
  domains: Record<string, number>;
  total_requests: number;
  total_blocked: number;
  total_alerts: number;
};

export type BodyFile = {
  name: string;
  size: number;
};
