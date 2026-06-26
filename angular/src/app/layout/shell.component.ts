import { Component, inject } from '@angular/core';
import { Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { AuthService } from '@attome/base';

interface NavItem { label: string; icon: string; path: string; }

@Component({
  selector:   'app-shell',
  standalone: true,
  imports:    [RouterOutlet, RouterLink, RouterLinkActive],
  template: `
    <div class="shell">
      <!-- Sidebar -->
      <aside class="sidebar">
        <div class="logo">
          <span class="logo-mark">A</span>
          <span class="logo-text">Attome</span>
        </div>

        <nav class="nav">
          @for (item of navItems; track item.path) {
            <a [routerLink]="item.path"
               routerLinkActive="active"
               [routerLinkActiveOptions]="{ exact: item.path === '/' }"
               class="nav-item">
              <span class="nav-icon">{{ item.icon }}</span>
              <span>{{ item.label }}</span>
            </a>
          }
        </nav>

        <div class="sidebar-footer">
          <div class="user-chip">
            <span class="avatar">{{ initial() }}</span>
            <span class="user-id">{{ shortId() }}</span>
          </div>
          <button class="logout-btn" (click)="logout()">Sign out</button>
        </div>
      </aside>

      <!-- Main content -->
      <main class="content">
        <router-outlet />
      </main>
    </div>
  `,
  styles: [`
    .shell   { display: flex; height: 100vh; overflow: hidden; }

    /* Sidebar */
    .sidebar { width: 220px; min-width: 220px; background: #1e1b4b; color: #c7d2fe;
               display: flex; flex-direction: column; padding: 0; }
    .logo    { display: flex; align-items: center; gap: 10px; padding: 20px 16px 16px;
               border-bottom: 1px solid #312e81; }
    .logo-mark { width: 32px; height: 32px; border-radius: 8px; background: #6366f1;
                 color: #fff; display: flex; align-items: center; justify-content: center;
                 font-weight: 700; font-size: 16px; }
    .logo-text { font-size: 18px; font-weight: 700; color: #fff; }

    .nav      { flex: 1; padding: 12px 8px; display: flex; flex-direction: column; gap: 2px; }
    .nav-item { display: flex; align-items: center; gap: 10px; padding: 9px 10px;
                border-radius: 8px; font-size: 13px; font-weight: 500; color: #a5b4fc;
                transition: background 0.15s; text-decoration: none; }
    .nav-item:hover  { background: #312e81; color: #e0e7ff; }
    .nav-item.active { background: #4338ca; color: #fff; }
    .nav-icon { font-size: 16px; width: 20px; text-align: center; }

    .sidebar-footer { padding: 12px; border-top: 1px solid #312e81; }
    .user-chip { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
    .avatar    { width: 28px; height: 28px; border-radius: 50%; background: #4338ca;
                 color: #fff; display: flex; align-items: center; justify-content: center;
                 font-size: 12px; font-weight: 600; }
    .user-id   { font-size: 11px; color: #818cf8; font-family: monospace; }
    .logout-btn { width: 100%; padding: 7px; background: transparent; border: 1px solid #4338ca;
                  color: #a5b4fc; border-radius: 6px; cursor: pointer; font-size: 12px; }
    .logout-btn:hover { background: #312e81; }

    /* Content */
    .content { flex: 1; overflow-y: auto; padding: 32px; background: #f9fafb; }
  `],
})
export class ShellComponent {
  private readonly auth   = inject(AuthService);
  private readonly router = inject(Router);

  readonly navItems: NavItem[] = [
    { label: 'Dashboard', icon: '⊞', path: '/dashboard' },
    { label: 'Entities',  icon: '◫', path: '/entity/contact' },
  ];

  initial()  { return (this.auth.userId() ?? 'U').slice(0, 1).toUpperCase(); }
  shortId()  { return (this.auth.userId() ?? '').slice(0, 8); }

  logout(): void {
    this.auth.logout().subscribe();
  }
}
