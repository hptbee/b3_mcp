import { Controller, Get, Post } from '@nestjs/common';

@Controller('/api/users')
export class UsersController {
  @Post()
  createUser() {
    return { status: 'created' };
  }

  @Get(':id')
  lookupUser() {
    return { id: 'user-1' };
  }
}
