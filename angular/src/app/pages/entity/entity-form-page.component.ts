import { Component, inject, input } from '@angular/core';
import { Router } from '@angular/router';
import { EntityFormComponent } from '@attome/xrm';

@Component({
  selector:   'app-entity-form-page',
  standalone: true,
  imports:    [EntityFormComponent],
  template: `
    <xrm-entity-form
      [entityName]="entityName()"
      [recordId]="recordId()"
      (saved)="onSaved()"
      (cancelled)="onCancel()"
    />
  `,
})
export class EntityFormPageComponent {
  private readonly router = inject(Router);

  readonly entityName = input('', { alias: 'name' });
  readonly recordId = input<string | null>(null, { alias: 'id' });

  onSaved(): void  { this.router.navigate(['/entity', this.entityName()]); }
  onCancel(): void { this.router.navigate(['/entity', this.entityName()]); }
}
