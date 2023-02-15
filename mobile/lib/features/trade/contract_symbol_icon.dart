import 'package:flutter/material.dart';
import 'package:flutter_svg/svg.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';

class ContractSymbolIcon extends StatelessWidget {
  const ContractSymbolIcon(
      {this.height = 30.0,
      this.width = 30.0,
      this.paddingUsd = const EdgeInsets.only(left: 20.0),
      super.key,
      this.contractSymbol = ContractSymbol.btcusd});

  final double width;
  final double height;
  final EdgeInsets paddingUsd;
  final ContractSymbol contractSymbol;

  @override
  Widget build(BuildContext context) {
    switch (contractSymbol) {
      default:
        return Stack(children: [
          Container(
            padding: paddingUsd,
            child: SizedBox(
                height: height, width: width, child: SvgPicture.asset("assets/USD_logo.svg")),
          ),
          SizedBox(
              height: height, width: width, child: SvgPicture.asset("assets/Bitcoin_logo.svg")),
        ]);
    }
  }
}
