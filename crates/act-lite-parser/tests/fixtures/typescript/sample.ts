// Sample TypeScript file for snapshot testing

import { Logger } from './logger';

export interface User {
  id: number;
  name: string;
}

export class UserService {
  private logger: Logger;

  constructor(logger: Logger) {
    this.logger = logger;
  }

  async getUser(id: number): Promise<User> {
    this.logger.log(`Fetching user ${id}`);
    // Implementation here
    return { id, name: 'Test' };
  }
}

export function createDefaultUser(): User {
  return { id: 0, name: 'Anonymous' };
}
