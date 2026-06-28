import { inject } from '@angular/core';
import { CanActivateFn, Router, Routes } from '@angular/router';
import { AuthService, authGuard, LoginPageComponent } from '@attome/base';

const xrmEntryGuard: CanActivateFn = () => {
  const auth   = inject(AuthService);
  const router = inject(Router);
  return router.createUrlTree(auth.isLoggedIn() ? ['/xrm/dashboard'] : ['/xrm/login']);
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
    data: { successRoute: '/xrm/dashboard', backRoute: '/', pageTitle: 'XRM Access', pageSubtitle: 'Sign in to the Attome Enterprise Platform' },
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
    path: 'xrm',
    loadComponent: () => import('@attome/xrm').then(m => m.ShellComponent),
    children: [
      {
        path: 'dashboard',
        canActivate: [authGuard],
        loadComponent: () => import('@attome/xrm').then(m => m.DashboardComponent),
      },
      {
        path: 'entity/:name',
        canActivate: [authGuard],
        loadComponent: () => import('@attome/xrm').then(m => m.EntityListPageComponent),
      },
      {
        path: 'entity/:name/new',
        canActivate: [authGuard],
        loadComponent: () => import('@attome/xrm').then(m => m.EntityFormPageComponent),
      },
      {
        path: 'entity/:name/:id/edit',
        canActivate: [authGuard],
        loadComponent: () => import('@attome/xrm').then(m => m.EntityFormPageComponent),
      },
    ],
  },
  {
    path: 'xrm/admin',
    loadComponent: () => import('@attome/xrm').then(m => m.ShellComponent),
    children: [
      {
        path: 'entities',
        canActivate: [authGuard],
        loadComponent: () => import('@attome/xrm').then(m => m.EntityManagerComponent),
      },
      {
        path: 'entities/:id',
        canActivate: [authGuard],
        loadComponent: () => import('@attome/xrm').then(m => m.EntityEditorComponent),
      },
    ],
  },
  { path: '**', redirectTo: '' },
];
