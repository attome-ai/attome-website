import { AfterViewInit, Component, effect, inject, signal } from '@angular/core';
import { Router } from '@angular/router';
import { AuthService, ApiError, ATTOME_CONFIG } from '@attome/base';
import { AuthModalService } from '../../services/auth-modal.service';

declare const google: any;

@Component({
  selector: 'app-auth-modal',
  standalone: true,
  imports: [],
  templateUrl: './auth-modal.component.html',
  styleUrl: './auth-modal.component.css',
})
export class AuthModalComponent implements AfterViewInit {
  private readonly auth     = inject(AuthService);
  private readonly router   = inject(Router);
  private readonly modalSvc = inject(AuthModalService);
  private readonly config   = inject(ATTOME_CONFIG);

  readonly isOpen  = this.modalSvc.isOpen;
  readonly loading = signal(false);
  readonly error   = signal<string | null>(null);

  private scriptLoaded = false;
  readonly googleBtnId = 'attome-google-btn';

  constructor() {
    effect(() => {
      if (this.isOpen()) {
        this.error.set(null);
        setTimeout(() => this.renderGoogleButton(), 80);
      }
    });
  }

  ngAfterViewInit(): void {
    if (this.config.googleClientId) this.loadGoogleScript();
  }

  close(): void { this.modalSvc.close(); }

  closeOnBackdrop(event: MouseEvent): void {
    if ((event.target as HTMLElement).classList.contains('modal-backdrop')) this.close();
  }

  renderGoogleButton(): void {
    if (!this.scriptLoaded || typeof google === 'undefined') return;
    const el = document.getElementById(this.googleBtnId);
    if (!el) return;
    el.innerHTML = '';
    google.accounts.id.renderButton(el, {
      theme: 'filled_black', size: 'large',
      width: el.offsetWidth || 308, type: 'standard',
      shape: 'rectangular', text: 'signin_with', logo_alignment: 'left',
    });
  }

  private loadGoogleScript(): void {
    if (typeof google !== 'undefined') { this.scriptLoaded = true; this.initGoogle(); return; }
    const s = document.createElement('script');
    s.src = 'https://accounts.google.com/gsi/client';
    s.async = true; s.defer = true;
    s.onload = () => { this.scriptLoaded = true; this.initGoogle(); };
    document.head.appendChild(s);
  }

  private initGoogle(): void {
    if (typeof google === 'undefined' || !this.config.googleClientId) return;
    google.accounts.id.initialize({
      client_id: this.config.googleClientId,
      callback: (r: { credential: string }) => this.handleCredential(r),
      auto_select: false,
      cancel_on_tap_outside: false,
    });
    this.renderGoogleButton();
  }

  private handleCredential(r: { credential: string }): void {
    if (!r.credential) return;
    this.loading.set(true);
    this.error.set(null);
    this.auth.loginWithGoogle({ credential: r.credential }).subscribe({
      next: () => { this.close(); this.router.navigateByUrl('/dashboard'); },
      error: (err: ApiError) => {
        this.loading.set(false);
        this.error.set(err.message ?? 'Google sign-in failed.');
      },
    });
  }
}
