import { inject } from '@angular/core';
import { CanActivateFn, Router, Routes } from '@angular/router';
import { AuthService, authGuard, LoginPageComponent } from '@attome/base';

const xrmEntryGuard: CanActivateFn = () => {
  const auth   = inject(AuthService);
  const router = inject(Router);
  return router.createUrlTree(auth.isLoggedIn() ? ['/dashboard'] : ['/xrm/login']);
};

export const routes: Routes = [
  {
    path: '',
    pathMatch: 'full',
    loadComponent: () => import('./pages/landing/landing.component').then(m => m.LandingComponent),
  },
  {
    path: 'xrm',
    pathMatch: 'full',
    canActivate: [xrmEntryGuard],
    loadComponent: () => import('./pages/landing/landing.component').then(m => m.LandingComponent),
  },
  {
    path: 'xrm/login',
    component: LoginPageComponent,
    data: { successRoute: '/dashboard', backRoute: '/', pageTitle: 'XRM Access', pageSubtitle: 'Sign in to the Attome Enterprise Platform' },
  },
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
