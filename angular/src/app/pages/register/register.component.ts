import { Component, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { Router, RouterLink } from '@angular/router';
import { AuthService, ApiError } from '@attome/base';

@Component({
  selector:   'app-register',
  standalone: true,
  imports:    [ReactiveFormsModule, RouterLink],
  template: `
    <div class="auth-page">
      <div class="auth-card">
        <div class="auth-logo">
          <span class="logo-mark">A</span>
          <h1>Attome</h1>
        </div>

        <h2>Create account</h2>
        <p class="subtitle">Get started for free</p>

        <form [formGroup]="form" (ngSubmit)="submit()">
          <div class="field">
            <label for="email">Email</label>
            <input id="email" type="email" formControlName="email"
                   placeholder="you@example.com" autocomplete="email" />
          </div>

          <div class="field">
            <label for="password">Password</label>
            <input id="password" type="password" formControlName="password"
                   placeholder="Min 8 characters" autocomplete="new-password" />
          </div>

          @if (error()) {
            <p class="error-msg">{{ error() }}</p>
          }

          <button type="submit" class="btn-submit" [disabled]="loading()">
            {{ loading() ? 'Creating account…' : 'Create account' }}
          </button>
        </form>

        <p class="auth-link">
          Already have an account? <a routerLink="/login">Sign in</a>
        </p>
      </div>
    </div>
  `,
  styles: [`@import '../auth-shared.css';`],
})
export class RegisterComponent {
  private readonly auth   = inject(AuthService);
  private readonly router = inject(Router);
  private readonly fb     = inject(FormBuilder);

  readonly loading = signal(false);
  readonly error   = signal<string | null>(null);

  readonly form = this.fb.group({
    email:    ['', [Validators.required, Validators.email]],
    password: ['', [Validators.required, Validators.minLength(8)]],
  });

  submit(): void {
    if (this.form.invalid) { this.form.markAllAsTouched(); return; }
    this.loading.set(true);
    this.error.set(null);

    const { email, password } = this.form.value;
    this.auth.register({ email: email!, password: password! }).subscribe({
      next: () => this.router.navigateByUrl('/dashboard'),
      error: (err: ApiError) => {
        this.loading.set(false);
        this.error.set(err.status === 409 ? 'Email already registered.' : err.message);
      },
    });
  }
}
