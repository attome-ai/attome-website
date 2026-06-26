import { Routes } from '@angular/router';
import { authGuard } from '@attome/base';

export const routes: Routes = [
  {
    path: 'login',
    loadComponent: () => import('./pages/login/login.component').then(m => m.LoginComponent),
  },
  {
    path: 'register',
    loadComponent: () => import('./pages/register/register.component').then(m => m.RegisterComponent),
  },
  {
    path: '',
    canActivate: [authGuard],
    loadComponent: () => import('./layout/shell.component').then(m => m.ShellComponent),
    children: [
      {
        path: '',
        redirectTo: 'dashboard',
        pathMatch: 'full',
      },
      {
        path: 'dashboard',
        loadComponent: () => import('./pages/dashboard/dashboard.component').then(m => m.DashboardComponent),
      },
      {
        path: 'entity/:name',
        loadComponent: () => import('./pages/entity/entity-list-page.component').then(m => m.EntityListPageComponent),
      },
      {
        path: 'entity/:name/new',
        loadComponent: () => import('./pages/entity/entity-form-page.component').then(m => m.EntityFormPageComponent),
      },
      {
        path: 'entity/:name/:id/edit',
        loadComponent: () => import('./pages/entity/entity-form-page.component').then(m => m.EntityFormPageComponent),
      },
    ],
  },
  { path: '**', redirectTo: '' },
];
