import 'package:bitcoin_icons/bitcoin_icons.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/model.dart';

class TradeScreen extends StatefulWidget {
  static const route = "/trade";

  // TODO: get price from service
  final Price price = Price(41129);

  // for now just to avoid division by 0 errors, later we should introduce a maintenance margin
  final double maintenanceMargin = 0.001;

  TradeScreen({super.key});

  @override
  State<TradeScreen> createState() => _TradeScreenState();
}

class _TradeScreenState extends State<TradeScreen> {
  Price? _price;
  Usd? _quantity;
  Leverage _leverage = Leverage(1);
  final TextEditingController _marginController = TextEditingController();
  final TextEditingController _liquidationPriceController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _price = widget.price;
    _quantity = Usd(100);

    if (_quantity != null && _price != null) {
      _marginController.text = calculateMargin(_quantity!, _price!, _leverage).formatted();
      _liquidationPriceController.text =
          calculateLiquidationPrice(_quantity!, _price!, _leverage, widget.maintenanceMargin)
              .formatted();
    }
  }

  @override
  Widget build(BuildContext context) {
    const spaceBetweenRows = SizedBox(height: 10);
    return Column(
      mainAxisAlignment: MainAxisAlignment.start,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        SizedBox(
          width: 400,
          child: Column(
            children: [
              const SizedBox(height: 20),
              Align(
                alignment: AlignmentDirectional.centerEnd,
                child: AmountInputField(
                  value: _quantity,
                  enabled: true,
                  label: "Quantity",
                  textAlign: TextAlign.right,
                  suffixIcon: const Icon(FontAwesomeIcons.dollarSign),
                  onChanged: (quantity) => setState(() {
                    _quantity = Usd.parseString(quantity);
                    if (_quantity != null && _price != null) {
                      _marginController.text =
                          calculateMargin(_quantity!, _price!, _leverage).formatted();
                      _liquidationPriceController.text = calculateLiquidationPrice(
                              _quantity!, _price!, _leverage, widget.maintenanceMargin)
                          .formatted();
                    }
                  }),
                ),
              ),
              spaceBetweenRows,
              Align(
                alignment: AlignmentDirectional.centerEnd,
                child: AmountInputField(
                  value: _leverage,
                  enabled: true,
                  label: "Leverage",
                  textAlign: TextAlign.right,
                  onChanged: (leverage) => setState(() {
                    _leverage = Leverage(int.parse(leverage));
                    if (_quantity != null && _price != null) {
                      _marginController.text =
                          calculateMargin(_quantity!, _price!, _leverage).formatted();
                      _liquidationPriceController.text = calculateLiquidationPrice(
                              _quantity!, _price!, _leverage, widget.maintenanceMargin)
                          .formatted();
                    }
                  }),
                ),
              ),
              spaceBetweenRows,
              Align(
                alignment: AlignmentDirectional.centerEnd,
                child: AmountInputField(
                  enabled: false,
                  label: "Margin",
                  suffixIcon: const Icon(BitcoinIcons.satoshi_v1),
                  textAlign: TextAlign.right,
                  controller: _marginController,
                ),
              ),
              spaceBetweenRows,
              Align(
                alignment: AlignmentDirectional.centerEnd,
                child: AmountInputField(
                  enabled: false,
                  label: "Liquidation",
                  suffixIcon: const Icon(FontAwesomeIcons.dollarSign),
                  textAlign: TextAlign.right,
                  controller: _liquidationPriceController,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

Amount calculateMargin(Usd quantity, Price price, Leverage leverage) {
  return Amount.fromBtc(quantity.asDouble / (price.asDouble * leverage.asDouble));
}

Amount calculateLiquidationPrice(
    Usd quantity, Price price, Leverage leverage, double maintenanceMargin) {
  return Amount((price.asDouble * leverage.asDouble) ~/
      (leverage.asDouble - 1.0 + (maintenanceMargin * leverage.asDouble)));
}
