# Maintaining the Dune Linux VM from Windows

Prepared after a read-only inspection on September 13, 2026.

This guide covers the Alpine Linux VM used by Funcom's Hyper-V server package.
The inspection did not install updates, change server settings, restart anything,
or access the Dune database. The commands in the **installation** section below
are instructions for a future maintenance window; they have not been executed.

## Public archive scope

Host inventory, package counts, resource measurements, authentication settings,
and network-exposure findings have been removed. This is a dated procedure,
not a statement of any server's present security or update status.

## Recommended maintenance policy

Start with a supervised, manual update within Alpine 3.23, using a recovery copy
of the VM. Then review package updates monthly and address relevant urgent
security fixes sooner. Keep using Funcom's supplied script for game updates and
game backups.

Keep installation and reboots in a planned maintenance window. Review current
vendor guidance and the support status of both main and community repositories
before using this historical Alpine 3.23 procedure. Do not mix branches or change
them to edge/latest-stable during routine maintenance.
[Alpine's support policy](https://alpinelinux.org/releases/)

## First update: Windows-friendly runbook

Run commands one at a time. If a command reports an error, stop at that step and
keep the message for troubleshooting. Do not proceed to a reboot after a failed
kernel/package installation.

### 1. Set aside downtime and establish recovery

Choose a window outside the manager's scheduled daily restart. Finish any
Funcom game update separately and confirm the world works first.

1. Use Funcom's `battlegroup.bat` backup option and confirm it completed. Keep a
   copy outside the VM. The manager does not perform this backup.
2. Stop the battlegroup normally through the manager or Funcom's script. Wait
   for the normal stop process to finish.
3. In Hyper-V Manager, right-click the VM and choose **Shut Down**. Wait until
   its state is **Off**; **Turn Off** is the forced-power option.
4. Right-click the stopped VM and choose **Export**. Pick a location with
   sufficient space, preferably on another drive, and wait for export to finish.
   This preserves the Linux installation and virtual disks as well as the game.
5. Start the VM again. Keep the battlegroup stopped during OS maintenance.

Hyper-V exports include VM configuration, virtual disks, and checkpoint files.
The stopped-VM procedure above provides a straightforward recovery point.
[Microsoft's export/import instructions](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/deploy/export-and-import-virtual-machines)

### 2. Connect from Windows PowerShell

Find the current **Host** address in the manager's server profile. Open Windows
PowerShell and run:

```powershell
$vmAddress = Read-Host 'Enter the VM IP address from the manager Host field'
$vmUser = Read-Host 'Enter the SSH user from your server profile'
$vmKey = Read-Host 'Enter the SSH key path from your server profile'
ssh -i $vmKey -o StrictHostKeyChecking=yes "$vmUser@$vmAddress"
```

Use your own profile values. Do not dismiss an SSH host-key mismatch; verify the VM identity first.

Once connected, commands run **inside Linux**. `sudo` means run a command with
administrator privileges, similar to Run as administrator on Windows. Absolute
paths below avoid this VM's limited non-interactive command search path.

### 3. Confirm the branch and disk space

These commands only read information:

```sh
cat /etc/alpine-release
cat /etc/apk/repositories
df -h / /boot/efi
uname -r
```

For this runbook, active repository lines must still use `/alpine/v3.23/main`
and `/alpine/v3.23/community`. Stop and review the plan if the branch or package
sources differ. Leave repository configuration unchanged during this update.

### 4. Install the package-manager update, then preview the rest

**These next two commands make changes.** Run them only after the recovery and
downtime steps above. The first refreshes the list of available packages; the
second upgrades Alpine's package manager and any required dependencies.

```sh
sudo /sbin/apk update
```

```sh
sudo /sbin/apk add --upgrade apk-tools
```

Now preview the remaining package updates without installing them:

```sh
sudo /sbin/apk upgrade --simulate
```

Stop if the package tool reports unresolved errors, unexpected removals,
a branch change, or a continuing simulation warning.

This sequence follows Alpine's package-update mechanism; the first package-manager
step refreshes the package manager before the remaining update preview. Updating packages is
not a guarantee that Funcom has tested that combination.
[Alpine's package-management documentation](https://docs.alpinelinux.org/user-handbook/0.1a/Working/apk.html)

### 5. Install the reviewed Linux updates

**This command installs updates immediately:**

```sh
sudo /sbin/apk upgrade
```

Wait for it to finish successfully. Do not close the SSH window while packages
or kernel boot files are being installed. If you see a failure involving
`mkinitfs`, `initramfs`, GRUB, disk space, or a package script, stop and investigate
before rebooting.

Check for configuration files needing review:

```sh
sudo find /etc -name '*.apk-new' -type f
```

No output means none were found. If paths appear, review those files before
continuing; do not automatically replace existing configuration. Alpine preserves
locally modified configuration and can supply updated defaults as `.apk-new`.

These commands update the VM's installed Alpine packages. They do not run
Funcom's game updater, replace the separately installed K3s binary, or patch the
software inside existing container images.

### 6. Reboot the VM and reconnect

After successful installation and any configuration review:

```sh
sudo /sbin/reboot
```

The SSH connection will close. That is expected. Wait for the VM to boot, then
run the PowerShell SSH command again. If the address changed, rediscover it
through the usual Funcom/Hyper-V tools rather than repeatedly trying an old IP.

The VM reboot activates the new kernel and restarts system services. It causes
server downtime; rebooting only the battlegroup does not activate a new kernel.

### 7. Verify before letting players back in

Inside the VM, run:

```sh
cat /etc/alpine-release
uname -r
sudo /sbin/rc-service k3s status
sudo /sbin/rc-service dune-server-service status
sudo /usr/local/bin/kubectl --request-timeout=15s get nodes
sudo /usr/local/bin/kubectl --request-timeout=15s get pods -A
```

Compare the running versions with the packages actually installed during your
maintenance window. The node and expected services should return to healthy.

Start the battlegroup through the normal manager or Funcom controls. Refresh
the dashboard, confirm normal services/maps, and join the game to confirm your
world and character are present. Pods belonging to an intentionally stopped
battlegroup can be absent until it is started. Old successful backup jobs can
show Completed/Succeeded and do not need deletion.

Keep the export until this validation passes. Package updates are not confirmed
successful merely because the SSH connection works.

### If something goes wrong

Use Hyper-V **Connect** to inspect the VM console if SSH does not return. Preserve
the error and the updated VM disks while deciding whether recovery is needed.

The full export is the recovery path if the installation cannot be repaired.
Restore/import it with the affected VM powered off, using Microsoft's guide or
assistance. Do not run two copies of the same Dune world simultaneously. Restoring
an old virtual disk also restores the world state from that time; progress made
after the recovery copy would be lost. Avoid ad-hoc package downgrades as a
substitute for a planned restore.

## Separate follow-up: Funcom's infrastructure and network access

Host OS updates do not update separately installed Kubernetes components or
software inside game/container images. Use Funcom's supported infrastructure
upgrade path; do not replace arbitrary images or rerun generic installers.

Keep administration private, review router forwarding and IPv6 exposure, and
verify SSH trust and unique credentials. Preserve access needed by Funcom's
script and the manager. Do not apply generic firewall rules without accounting
for Kubernetes networking. No host-specific network or authentication findings
are included in this public copy.

## Scope and ongoing use

This is a maintenance guide, not an app feature or automatic updater. No app
version bump, release, SQL, migration, or PR deployment is required for it.
Keep it in `docs/linux-vm-maintenance.md` so future work in this repository can
reuse it. Rediscover the current VM/profile and rerun the read-only checks before
using dated versions or package counts as a basis for future changes.
