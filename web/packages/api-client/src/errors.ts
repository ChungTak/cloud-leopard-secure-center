export interface ProblemDetails {
  type?: string;
  title?: string;
  status?: number;
  detail?: string;
  instance?: string;
}

export class ApiError extends Error {
  status: number;
  code: string;
  detail?: string;
  instance?: string;
  retryAfter?: number;

  constructor(
    status: number,
    code: string,
    detail?: string,
    instance?: string,
    retryAfter?: number,
  ) {
    super(detail ?? titleForStatus(status) ?? code);
    this.status = status;
    this.code = code;
    this.detail = detail;
    this.instance = instance;
    this.retryAfter = retryAfter;
  }

  static async fromResponse(response: Response): Promise<ApiError> {
    const retryAfter = parseRetryAfter(response.headers.get('Retry-After'));
    const problem = await parseProblemJson(response);
    const status = problem.status ?? response.status;
    const code =
      problem.type ??
      (response.status >= 400 && response.status < 500
        ? `http-${response.status}`
        : 'unknown');
    return new ApiError(
      status,
      code,
      problem.detail ?? problem.title,
      problem.instance,
      retryAfter,
    );
  }
}

async function parseProblemJson(response: Response): Promise<ProblemDetails> {
  try {
    const body = (await response.clone().json()) as ProblemDetails;
    if (body && typeof body === 'object') return body;
  } catch {
    // not JSON
  }
  return {};
}

function parseRetryAfter(value: string | null): number | undefined {
  if (value == null) return undefined;
  const seconds = Number(value);
  if (!Number.isNaN(seconds)) return seconds;
  const date = new Date(value).getTime();
  if (!Number.isNaN(date))
    return Math.max(0, Math.ceil((date - Date.now()) / 1000));
  return undefined;
}

function titleForStatus(status: number): string | undefined {
  const titles: Record<number, string> = {
    401: 'Unauthorized',
    403: 'Forbidden',
    404: 'Not Found',
    409: 'Conflict',
    412: 'Precondition Failed',
    429: 'Too Many Requests',
  };
  return titles[status];
}
