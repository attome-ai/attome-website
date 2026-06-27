import { ApplicationConfig } from '@angular/core';
import { provideRouter, withComponentInputBinding } from '@angular/router';
import { provideAnimationsAsync } from '@angular/platform-browser/animations/async';
import { provideAttomeBase } from '@attome/base';
import { environment } from '../environments/environment';
import { routes } from './app.routes';

export const appConfig: ApplicationConfig = {
  providers: [
    provideRouter(routes, withComponentInputBinding()),
    provideAnimationsAsync(),
    provideAttomeBase({ apiUrl: environment.apiUrl, googleClientId: environment.googleClientId, portalKey: 'xrm', loginRoute: '/xrm/login' }),
  ],
};
