export const routes = [
  {
    path: 'users/:id',
    loadComponent: () => import('./user-card.component').then(m => m.UserCardComponent),
  },
];

export class UserCardComponent {
  title = 'User lookup route';
}
