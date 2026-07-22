export interface HealthResponse {
  status: string;
}

export class ApiClient {
  constructor(public readonly baseUrl: string) {}

  async health(): Promise<HealthResponse> {
    return { status: 'ok' };
  }
}
