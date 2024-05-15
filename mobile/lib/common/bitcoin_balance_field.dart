import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/application/util.dart';

class BitcoinBalanceField extends StatelessWidget {
  final Amount bitcoinBalance;
  final double? fontSize;

  const BitcoinBalanceField({super.key, required this.bitcoinBalance, this.fontSize = 28.0});

  @override
  Widget build(BuildContext context) {
    var (leading, balance) = getFormattedBalance(bitcoinBalance.toInt);

    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Text(leading,
            style: TextStyle(
              color: Colors.grey,
              fontSize: fontSize,
              fontWeight: FontWeight.bold,
            )),
        Text(balance,
            style: TextStyle(
              color: Colors.black87,
              fontSize: fontSize,
              fontWeight: FontWeight.bold,
            )),
        Icon(Icons.currency_bitcoin, size: fontSize, color: tenTenOnePurple),
      ],
    );
  }
}
