import 'package:flutter/material.dart';
import 'package:flutter_svg/svg.dart';

class BtcUsdTradingPairImage extends StatelessWidget {
  const BtcUsdTradingPairImage(
      {this.height = 30.0,
      this.width = 30.0,
      this.paddingUsd = const EdgeInsets.only(left: 20.0),
      super.key});

  final double width;
  final double height;
  final EdgeInsets paddingUsd;

  @override
  Widget build(BuildContext context) {
    return Stack(children: [
      Container(
        padding: paddingUsd,
        child:
            SizedBox(height: height, width: width, child: SvgPicture.asset("assets/USD_logo.svg")),
      ),
      SizedBox(height: height, width: width, child: SvgPicture.asset("assets/Bitcoin_logo.svg")),
    ]);
  }
}
