import { Component } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { AuthModalComponent } from './components/auth-modal/auth-modal.component';

@Component({
  selector:   'app-root',
  standalone: true,
  imports:    [RouterOutlet, AuthModalComponent],
  template:   `<router-outlet /><app-auth-modal />`,
})
export class AppComponent {}
