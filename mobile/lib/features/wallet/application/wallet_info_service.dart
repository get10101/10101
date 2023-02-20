import 'package:f_logs/f_logs.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/domain/payment_flow.dart';
import 'package:get_10101/features/wallet/domain/transaction.dart';
import 'package:get_10101/features/wallet/domain/wallet_balances.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/ffi.dart' as rust;

class WalletInfoService {
   Future<WalletInfo?> getWalletInfo() async {
     try {
       final walletInfo = await rust.api.refreshWalletInfo();
       FLog.trace(text: 'Successfully retrieved wallet info');
       return WalletInfo(
         balances: WalletBalances(
           onChain: Amount(walletInfo.balances.onChain),
           lightning: Amount(walletInfo.balances.lightning)
          ),
         history: walletInfo.history.map((tx) {
           return Transaction(
               address: tx.address,
               flow: tx.flow == rust.PaymentFlow.Outbound ? PaymentFlow.outbound : PaymentFlow.inbound,
               amount: Amount(tx.amountSats),
               walletType: tx.walletType == rust.WalletType.Lightning ? WalletType.lightning : WalletType.onChain,
           );
         }).toList(),
       );
     } catch (error) {
       FLog.error(text: "Failed to get wallet info: $error");
       return null;
     }
   }
}
