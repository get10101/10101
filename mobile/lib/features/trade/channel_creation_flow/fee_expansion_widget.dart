import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/value_data_row.dart';

class FeeExpansionTile extends StatefulWidget {
  final String label;
  final Amount value;
  final Amount orderMatchingFee;
  final Amount fundingTxFee;
  final Amount channelFeeReserve;

  const FeeExpansionTile({
    super.key,
    required this.label,
    required this.value,
    required this.orderMatchingFee,
    required this.fundingTxFee,
    required this.channelFeeReserve,
  });

  @override
  State<FeeExpansionTile> createState() => _FeeExpansionTileState();
}

class _FeeExpansionTileState extends State<FeeExpansionTile> with SingleTickerProviderStateMixin {
  bool _isExpanded = false;
  late AnimationController _controller;
  late Animation<double> _expandAnimation;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 300),
    );
    _expandAnimation = CurvedAnimation(
      parent: _controller,
      curve: Curves.easeInOut,
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _toggleExpand() {
    setState(() {
      _isExpanded = !_isExpanded;
      if (_isExpanded) {
        _controller.forward();
      } else {
        _controller.reverse();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        GestureDetector(
          onTap: _toggleExpand,
          child: ValueDataRow(type: ValueType.amount, value: widget.value, label: 'Fee*'),
        ),
        SizeTransition(
          sizeFactor: _expandAnimation,
          child: Container(
            padding: const EdgeInsets.only(top: 4.0, left: 8.0, right: 8),
            color: Colors.grey[100], // You can change this to match your design
            child: SingleChildScrollView(
              child: GestureDetector(
                onTap: _toggleExpand,
                child: Column(
                  children: [
                    ValueDataRow(
                        type: ValueType.amount,
                        value: widget.orderMatchingFee,
                        label: 'Order matching fee'),
                    ValueDataRow(
                        type: ValueType.amount,
                        value: widget.fundingTxFee,
                        label: 'Funding tx fee'),
                    ValueDataRow(
                        type: ValueType.amount,
                        value: widget.channelFeeReserve,
                        label: 'Channel reserve fee')
                  ],
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
