# I/O coverage matrix — D01の調査契約

更新: 2026-11-02 JST。**全行の実stack観測はNOT RUN。** 表の処置は初期製品への保守的な設計提案であり、Windowsの実際の捕捉位置・実装済み対応を表さない。

| 経路 | 監視対象・落とし穴 | 初期方針 / labで必要な証拠 |
|---|---|---|
| ordinary cached/noncached write | application completionとstorage arrivalは別 | physical rangeを確定、全交差lineをforward前に失効。buffered/noncached/write-through各経路のsubmit/completionを相関 |
| paging / mapped write | pagefileを除外してもmapped fileのpaging I/Oは残る | mapped dirty pageとmetadataのlower writeを捕捉。通常writeだけの試験で代用しない |
| raw volume / physical disk | volume handleとPhysicalDrive handleは同じ経路とは限らない | volume内外range・partition offset・locked/offline対象の経路を別々に確認。volumeだけで網羅としない |
| DSM/TRIM | DEVICE_DSM_INPUT / range / whole-dataset、通常writeではない | size/offset/count/actionを検証。該当範囲または全sessionを失効。discard後のzeroを仮定しない |
| unknown DSM | NonDestructive bitだけで一般IOCTLまで安全と分類してはいけない | Microsoft DSM手順に従い未知のdestructive actionを安易にforwardしない。初期方針は拒否と診断 |
| zero/sparse operation | FSCTL_SET_ZERO_DATAをdisk filterがそのまま見るとは限らない | 実際のwrite/discard/controlとmetadataを追う。internal-useのDSM WriteZeroesを独自public probeで発行しない |
| offload copy | offload write destinationは通常payloadなしでも変更される | source offload-readとdestination-writeを区別。不明なdestructive offloadは拒否。未対応hardwareで失敗しただけならcoverage未確認 |
| SCSI pass-through | CDBの意味、DIRECT/EX、class/adapter routing | directionやDATA_INだけでnondestructiveとしない。未知commandは拒否/対象非対応。classを迂回する経路が残ればactivation不可 |
| ATA pass-through | SATA HDDにも専用command経路がある | SCSI/NVMeだけで済ませない。security/erase/vendor command等は初期allowlist外 |
| NVMe/protocol/adapter control | cache SSD側にも外部変更経路がある | originだけでなくcache device/partitionの専有とadapter境界を確認。disk filterが全adapter commandを見ると推測しない |
| resize/repartition/reformat | grow/layout変更が通常writeだけとは限らない | 変更前にhit/fill停止、drain、cold reset。drive letterの事後変化検出では不足。formatは複数操作のworkload |
| BypassIO | storage側IOCTLとfilesystem側FSCTLは別 | 初期非互換時はdocumented veto/診断を設計。minifilter用FltVetoBypassIoをdisk filterのAPIと誤用しない |
| offline/online/restore/reset/firmware | identityが同じでも内容は変わる | 全再入場をcold。未対応restore/vendor-management stackをallowlistに入れない |
| unconfigured / OS disk | class-wide登録だけでOS diskへ影響し得る | no-target boot、対象外I/O、失敗時回復を実証。対象判定前にcache controlを走らせない |

## BypassIOの限定

[Microsoft storage説明](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/bypassio)と[filesystem説明](https://learn.microsoft.com/windows-hardware/drivers/ifs/bypassio)が記載する現在の対象はclient Windows / NVMe / NTFS / noncached file reads。**bypassed writeと断定しない。** 指定filterを迂回するread経路があるため観測・性能・互換性には影響する。SATA HDDが現在の対象外でも将来の普遍的保証にはしない。hosted Windows Serverではclient機能をqualificationできない。

## 不明経路の処置

1. size/offset/action/identityの不明を「何もしないquery」と解釈しない。
2. 正しく扱えるmutationは失効とwriter trackingの後に原本へ転送し、結果/部分完了/flushの意味を保存する。
3. 拒否できる未知destructive controlは明示error、互換性制約を記録する。
4. quiesce+bypassを選ぶ場合、まず新規hit/fillを禁止し、既存read-result commitmentを排除し、物理leaseが完了するまでcache slotを再利用しない。epochを変えるだけでは足りない。
5. その停止障壁より下で発生し得る変更を捕捉/拒否できなければ、その構成ではactivationを拒否する。原本の全I/Oを無期限に凍結する万能filterを捏造しない。

## 一次資料（2026-11-02確認）

- [storage dispatch / SRB](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/storage-filter-driver-s-dispatch-routines)
- [mapped fileとpaging](https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/memory-mapped-files-in-a-file-system-filter-driver)
- [WriteFile / raw disk制約](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-writefile)
- [DSM overview](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/data-set-management-overview)、[action descriptions](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/device-dsm-action-descriptions)
- [SET_ZERO_DATA](https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/fsctl-set-zero-data)、[offloaded data transfers](https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/offloaded-data-transfers)
- [SCSI routing](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/bypassing-the-class-driver-with-scsi-pass-through-requests-to-storport)、[ATA support](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/ata-command-support)
- [NVMe access](https://learn.microsoft.com/en-us/windows/win32/fileio/working-with-nvme-devices)、[StorNVMe command support](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/stornvme-command-set-support)
- [filter installation](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/installing-a-filter-driver)、[ordering](https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/device-filter-driver-ordering)
- [grow partition](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntdddisk/ni-ntdddisk-ioctl_disk_grow_partition)、[set layout](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-ioctl_disk_set_drive_layout_ex)、[disk attributes](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-ioctl_disk_set_disk_attributes)

API名はqueryの実行許可でも、全OSで利用できる保証でもない。正確なheader/OS buildと実stack証拠はlab記録へ残す。
