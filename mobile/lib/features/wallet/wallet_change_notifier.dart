import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart' hide Flow;
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/application/wallet_info_service.dart';
import 'package:get_10101/features/wallet/domain/wallet_balances.dart';
import 'domain/wallet_info.dart';

class WalletChangeNotifier extends ChangeNotifier {
  final WalletInfoService service;
  WalletInfo walletInfo = WalletInfo(
      balances: WalletBalances(onChain: Amount(0), lightning: Amount(0)),
      history: List.empty(),
  );

  WalletChangeNotifier(this.service);

  void update(WalletInfo? walletInfo) {
    if (walletInfo == null) {
      // skip empty wallet info update.
      return;
    }
    this.walletInfo = walletInfo;

    FLog.trace(text: 'Successfully synced payment history');
    super.notifyListeners();
  }

  Future<void> refreshWalletInfo() async {
    update(await service.getWalletInfo());
  }

  Amount total() => Amount(onChain().sats + lightning().sats);
  Amount onChain() => walletInfo.balances.onChain;
  Amount lightning() => walletInfo.balances.lightning;
}
